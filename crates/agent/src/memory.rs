//! Evidence-backed memory. Hosts own source authorization and candidate generation.

mod storage;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::retrieval::SearchIndex;

#[cfg(feature = "memory-files")]
pub use storage::FileMemoryStore;
pub use storage::InMemoryStore;

const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Preference,
    Decision,
    Task,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub source: String,
    /// Host-provided fingerprint or source revision.
    pub revision: String,
    /// Host-interpreted reference, such as a section ID or line range.
    pub locator: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryProposal {
    /// Stable proposal ID. Repeating an identical proposal is idempotent.
    pub id: String,
    /// Identity of a claim, independent of its current value.
    pub key: String,
    pub kind: MemoryKind,
    pub title: String,
    pub content: String,
    /// Empty for an explicitly authored claim with no separate source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryState {
    Proposed,
    Confirmed,
    NeedsReview,
    Superseded,
    Rejected,
    Withdrawn,
}

impl MemoryState {
    fn is_current(self) -> bool {
        matches!(self, Self::Confirmed | Self::NeedsReview)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub proposal: MemoryProposal,
    pub state: MemoryState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryChange {
    /// Record the current source revision. None means the source was removed.
    ObserveSource {
        source: String,
        revision: Option<String>,
    },
    Propose(MemoryProposal),
    /// Acknowledge the current record for the key: None for a new key,
    /// Some(current_id) for a replacement or reconfirmation.
    Confirm {
        id: String,
        replaces: Option<String>,
    },
    Reject {
        id: String,
    },
    Withdraw {
        id: String,
    },
}

#[derive(Debug)]
pub enum MemoryError {
    Storage(std::io::Error),
    Invalid(&'static str),
    Stale {
        expected: u64,
        actual: u64,
    },
    DuplicateId(String),
    MissingRecord(String),
    InvalidTransition {
        id: String,
        state: MemoryState,
    },
    Conflict {
        key: String,
        current: Option<String>,
    },
    OutdatedEvidence(EvidenceRef),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(error) => write!(output, "Memory storage: {error}"),
            Self::Invalid(reason) => write!(output, "Invalid memory: {reason}"),
            Self::Stale { expected, actual } => write!(
                output,
                "Memory revision {expected} is stale; current revision is {actual}."
            ),
            Self::DuplicateId(id) => write!(
                output,
                "Memory proposal ID {id} already refers to different content."
            ),
            Self::MissingRecord(id) => write!(output, "No memory record with ID {id}."),
            Self::InvalidTransition { id, state } => write!(
                output,
                "The operation cannot be applied to {id} in state {state:?}."
            ),
            Self::Conflict { key, current } => write!(
                output,
                "Current memory for {key} differs from the reviewed record: {current:?}."
            ),
            Self::OutdatedEvidence(evidence) => write!(
                output,
                "Evidence from {} is no longer current.",
                evidence.source
            ),
        }
    }
}

impl std::error::Error for MemoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for MemoryError {
    fn from(error: std::io::Error) -> Self {
        Self::Storage(error)
    }
}

/// Host adapters must implement atomic compare-and-swap per namespace.
/// An absent namespace has revision 0; the first stored snapshot has revision 1.
pub trait MemoryStore: Send + Sync {
    fn load(&self, namespace: &str) -> Result<Option<Arc<MemorySnapshot>>, MemoryError>;
    fn compare_exchange(
        &self,
        namespace: &str,
        expected_revision: u64,
        snapshot: Arc<MemorySnapshot>,
    ) -> Result<(), MemoryError>;
}

#[derive(Clone, Serialize, Deserialize)]
struct DurableMemory {
    version: u32,
    namespace: String,
    revision: u64,
    sources: BTreeMap<String, String>,
    records: BTreeMap<String, MemoryRecord>,
}

/// Immutable view of one namespace. Reuse it throughout an agent turn to
/// keep the reference set and ranking consistent.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "DurableMemory")]
pub struct MemorySnapshot {
    #[serde(flatten)]
    data: DurableMemory,
    #[serde(skip)]
    recall: OnceLock<RecallIndex>,
}

impl TryFrom<DurableMemory> for MemorySnapshot {
    type Error = MemoryError;

    fn try_from(data: DurableMemory) -> Result<Self, Self::Error> {
        if data.version != FORMAT_VERSION {
            return Err(MemoryError::Invalid("unsupported snapshot format version"));
        }
        validate_namespace(&data.namespace)?;
        let mut current = BTreeSet::new();
        for (id, record) in &data.records {
            if *id != record.proposal.id {
                return Err(MemoryError::Invalid("record ID does not match its key"));
            }
            if record.state.is_current() && !current.insert(&record.proposal.key) {
                return Err(MemoryError::Invalid("multiple current records for one key"));
            }
            if record.state == MemoryState::Confirmed && data.outdated_evidence(record).is_some() {
                return Err(MemoryError::Invalid(
                    "confirmed record references an outdated source",
                ));
            }
            if let Some(replacement) = &record.superseded_by {
                if record.state != MemoryState::Superseded
                    || replacement == id
                    || data
                        .records
                        .get(replacement)
                        .is_none_or(|next| next.proposal.key != record.proposal.key)
                {
                    return Err(MemoryError::Invalid("invalid replacement record"));
                }
            } else if record.state == MemoryState::Superseded {
                return Err(MemoryError::Invalid("superseded record has no replacement"));
            }
        }
        Ok(Self {
            data,
            recall: OnceLock::new(),
        })
    }
}

#[derive(Default)]
struct RecallIndex {
    search: SearchIndex,
    ids: Vec<String>,
}

impl std::fmt::Debug for MemorySnapshot {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output
            .debug_struct("MemorySnapshot")
            .field("revision", &self.data.revision)
            .field("records", &self.data.records.len())
            .field("sources", &self.data.sources.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RecallQuery<'a> {
    pub text: &'a str,
    /// Empty means all kinds.
    pub kinds: &'a [MemoryKind],
    pub limit: usize,
    /// Budget for the UTF-8 JSON array of records, including evidence and
    /// delimiters. Below two bytes, no records are returned.
    pub max_bytes: usize,
}

impl RecallQuery<'_> {
    fn has_results(self) -> bool {
        self.limit > 0 && self.max_bytes >= 2 && !self.text.trim().is_empty()
    }
}

impl MemorySnapshot {
    pub fn namespace(&self) -> &str {
        &self.data.namespace
    }
    pub fn revision(&self) -> u64 {
        self.data.revision
    }
    pub fn get(&self, id: &str) -> Option<&MemoryRecord> {
        self.data.records.get(id)
    }

    pub fn source_revision(&self, source: &str) -> Option<&str> {
        self.data.sources.get(source).map(String::as_str)
    }

    pub fn records(&self) -> impl Iterator<Item = &MemoryRecord> {
        self.data.records.values()
    }

    /// Returns the current record, including one held for review.
    pub fn current(&self, key: &str) -> Option<&MemoryRecord> {
        self.records()
            .find(|record| record.state.is_current() && record.proposal.key == key)
    }

    /// Searches confirmed memories within this authorized namespace.
    pub fn recall(&self, query: RecallQuery<'_>) -> Vec<&MemoryRecord> {
        if !query.has_results() {
            return Vec::new();
        }
        self.rank(
            query,
            self.recall.get_or_init(|| self.build_index(|_| true)),
        )
    }

    /// Apply a host's record-level policy before indexing or ranking, so
    /// excluded records cannot affect which evidence the query returns.
    pub fn recall_where(
        &self,
        query: RecallQuery<'_>,
        allow: impl FnMut(&MemoryRecord) -> bool,
    ) -> Vec<&MemoryRecord> {
        if !query.has_results() {
            return Vec::new();
        }
        let index = self.build_index(allow);
        self.rank(query, &index)
    }

    fn build_index(&self, mut allow: impl FnMut(&MemoryRecord) -> bool) -> RecallIndex {
        let mut index = RecallIndex::default();
        for record in self
            .records()
            .filter(|record| record.state == MemoryState::Confirmed)
        {
            if allow(record) {
                let proposal = &record.proposal;
                index.search.add(&proposal.title, &proposal.content);
                index.ids.push(proposal.id.clone());
            }
        }
        index
    }

    fn rank(&self, query: RecallQuery<'_>, index: &RecallIndex) -> Vec<&MemoryRecord> {
        let mut remaining = query.max_bytes - 2;
        let mut result = Vec::new();
        for hit in index.search.search(query.text, usize::MAX) {
            let record = &self.data.records[&index.ids[hit]];
            if !query.kinds.is_empty() && !query.kinds.contains(&record.proposal.kind) {
                continue;
            }
            let Ok(encoded) = serde_json::to_vec(record) else {
                continue;
            };
            let bytes = encoded.len() + usize::from(!result.is_empty());
            if bytes > remaining {
                continue;
            }
            remaining -= bytes;
            result.push(record);
            if result.len() == query.limit {
                break;
            }
        }
        result
    }

    fn empty(namespace: &str) -> Self {
        Self {
            data: DurableMemory {
                version: FORMAT_VERSION,
                namespace: namespace.to_owned(),
                revision: 0,
                sources: BTreeMap::new(),
                records: BTreeMap::new(),
            },
            recall: OnceLock::new(),
        }
    }

    fn validate_namespace(&self, namespace: &str) -> Result<(), MemoryError> {
        if self.data.version != FORMAT_VERSION {
            return Err(MemoryError::Invalid("unsupported snapshot format version"));
        }
        if self.data.namespace != namespace {
            return Err(MemoryError::Invalid(
                "snapshot belongs to another namespace",
            ));
        }
        Ok(())
    }
}

/// Operates on one host-authorized namespace.
pub struct MemoryCatalog<'a> {
    store: &'a dyn MemoryStore,
    namespace: &'a str,
}

impl<'a> MemoryCatalog<'a> {
    pub fn new(store: &'a dyn MemoryStore, namespace: &'a str) -> Result<Self, MemoryError> {
        validate_namespace(namespace)?;
        Ok(Self { store, namespace })
    }

    pub fn snapshot(&self) -> Result<Arc<MemorySnapshot>, MemoryError> {
        let snapshot = self
            .store
            .load(self.namespace)?
            .unwrap_or_else(|| Arc::new(MemorySnapshot::empty(self.namespace)));
        snapshot.validate_namespace(self.namespace)?;
        Ok(snapshot)
    }

    /// Applies the whole batch or preserves the previous snapshot on failure.
    pub fn apply(
        &self,
        expected_revision: u64,
        changes: &[MemoryChange],
    ) -> Result<Arc<MemorySnapshot>, MemoryError> {
        let previous = self.snapshot()?;
        check_revision(expected_revision, previous.revision())?;
        let mut data = previous.data.clone();
        let mut index = ChangeIndex::default();
        for record in data.records.values() {
            index.add_evidence(&record.proposal);
            if record.state.is_current()
                && index
                    .current
                    .insert(record.proposal.key.clone(), record.proposal.id.clone())
                    .is_some()
            {
                return Err(MemoryError::Invalid("multiple current records for one key"));
            }
        }
        let mut changed = false;
        for change in changes {
            changed |= data.apply(change, &mut index)?;
        }
        if !changed {
            return Ok(previous);
        }
        data.revision = next_revision(expected_revision)?;
        let next = Arc::new(MemorySnapshot {
            data,
            recall: OnceLock::new(),
        });
        self.store
            .compare_exchange(self.namespace, expected_revision, next.clone())?;
        Ok(next)
    }
}

#[derive(Default)]
struct ChangeIndex {
    current: BTreeMap<String, String>,
    by_source: BTreeMap<String, BTreeSet<String>>,
}

impl ChangeIndex {
    fn add_evidence(&mut self, proposal: &MemoryProposal) {
        for evidence in &proposal.evidence {
            self.by_source
                .entry(evidence.source.clone())
                .or_default()
                .insert(proposal.id.clone());
        }
    }
}

impl DurableMemory {
    fn apply(
        &mut self,
        change: &MemoryChange,
        index: &mut ChangeIndex,
    ) -> Result<bool, MemoryError> {
        match change {
            MemoryChange::ObserveSource { source, revision } => {
                required(source)?;
                if let Some(revision) = revision {
                    required(revision)?;
                }
                if self.sources.get(source) == revision.as_ref() {
                    return Ok(false);
                }
                if let Some(revision) = revision {
                    self.sources.insert(source.clone(), revision.clone());
                } else {
                    self.sources.remove(source);
                }
                for id in index.by_source.get(source).into_iter().flatten() {
                    let record = self.records.get_mut(id).expect("indexed record must exist");
                    if record.state == MemoryState::Confirmed
                        && record.proposal.evidence.iter().any(|evidence| {
                            evidence.source == *source
                                && revision.as_ref() != Some(&evidence.revision)
                        })
                    {
                        // Remain in review if an old version later reappears.
                        record.state = MemoryState::NeedsReview;
                    }
                }
                Ok(true)
            }
            MemoryChange::Propose(proposal) => {
                for field in [&proposal.id, &proposal.key, &proposal.content] {
                    required(field)?;
                }
                for evidence in &proposal.evidence {
                    for field in [&evidence.source, &evidence.revision, &evidence.locator] {
                        required(field)?;
                    }
                }
                if let Some(record) = self.records.get(&proposal.id) {
                    return if record.proposal == *proposal {
                        Ok(false)
                    } else {
                        Err(MemoryError::DuplicateId(proposal.id.clone()))
                    };
                }
                index.add_evidence(proposal);
                self.records.insert(
                    proposal.id.clone(),
                    MemoryRecord {
                        proposal: proposal.clone(),
                        state: MemoryState::Proposed,
                        superseded_by: None,
                    },
                );
                Ok(true)
            }
            MemoryChange::Confirm { id, replaces } => {
                self.confirm(id, replaces.as_deref(), &mut index.current)
            }
            MemoryChange::Reject { id } => {
                let record = self
                    .records
                    .get_mut(id)
                    .ok_or_else(|| MemoryError::MissingRecord(id.clone()))?;
                if record.state == MemoryState::Rejected {
                    return Ok(false);
                }
                if record.state != MemoryState::Proposed {
                    return Err(invalid_transition(record));
                }
                record.state = MemoryState::Rejected;
                Ok(true)
            }
            MemoryChange::Withdraw { id } => {
                let record = self
                    .records
                    .get_mut(id)
                    .ok_or_else(|| MemoryError::MissingRecord(id.clone()))?;
                if record.state == MemoryState::Withdrawn {
                    return Ok(false);
                }
                if !record.state.is_current() {
                    return Err(invalid_transition(record));
                }
                index.current.remove(&record.proposal.key);
                record.state = MemoryState::Withdrawn;
                Ok(true)
            }
        }
    }

    fn outdated_evidence<'a>(&self, record: &'a MemoryRecord) -> Option<&'a EvidenceRef> {
        record
            .proposal
            .evidence
            .iter()
            .find(|evidence| self.sources.get(&evidence.source) != Some(&evidence.revision))
    }

    fn confirm(
        &mut self,
        id: &str,
        replaces: Option<&str>,
        current: &mut BTreeMap<String, String>,
    ) -> Result<bool, MemoryError> {
        let record = self
            .records
            .get(id)
            .ok_or_else(|| MemoryError::MissingRecord(id.to_owned()))?;
        if record.state != MemoryState::Proposed && !record.state.is_current() {
            return Err(invalid_transition(record));
        }
        let key = record.proposal.key.clone();
        let actual = current.get(&key).map(String::as_str);
        if actual != replaces {
            return Err(MemoryError::Conflict {
                key,
                current: actual.map(str::to_owned),
            });
        }
        if let Some(outdated) = self.outdated_evidence(record) {
            return Err(MemoryError::OutdatedEvidence(outdated.clone()));
        }
        if record.state == MemoryState::Confirmed {
            return Ok(false);
        }
        if let Some(prior) = replaces.filter(|&prior| prior != id) {
            let old = self
                .records
                .get_mut(prior)
                .expect("current record must exist");
            old.state = MemoryState::Superseded;
            old.superseded_by = Some(id.to_owned());
        }
        self.records
            .get_mut(id)
            .expect("confirmed record must exist")
            .state = MemoryState::Confirmed;
        current.insert(key, id.to_owned());
        Ok(true)
    }
}

fn invalid_transition(record: &MemoryRecord) -> MemoryError {
    MemoryError::InvalidTransition {
        id: record.proposal.id.clone(),
        state: record.state,
    }
}

fn required(value: &str) -> Result<(), MemoryError> {
    if value.trim().is_empty() {
        return Err(MemoryError::Invalid(
            "IDs, content, and evidence fields must be nonempty",
        ));
    }
    Ok(())
}

fn validate_namespace(namespace: &str) -> Result<(), MemoryError> {
    if !crate::portable_storage_id(namespace) {
        return Err(MemoryError::Invalid(
            "namespace must be 1–128 lowercase ASCII letters, digits, hyphens or underscores",
        ));
    }
    Ok(())
}

fn check_revision(expected: u64, actual: u64) -> Result<(), MemoryError> {
    if expected != actual {
        return Err(MemoryError::Stale { expected, actual });
    }
    Ok(())
}

fn next_revision(revision: u64) -> Result<u64, MemoryError> {
    revision
        .checked_add(1)
        .ok_or(MemoryError::Invalid("revision exhausted"))
}
