use std::sync::Arc;

use cove_agent::memory::{
    EvidenceRef, InMemoryStore, MemoryCatalog, MemoryChange, MemoryError, MemoryKind,
    MemoryProposal, MemorySnapshot, MemoryState, RecallQuery,
};

fn observe(source: &str, revision: Option<&str>) -> MemoryChange {
    MemoryChange::ObserveSource {
        source: source.to_owned(),
        revision: revision.map(str::to_owned),
    }
}

fn evidence(source: &str, revision: &str, locator: &str) -> EvidenceRef {
    EvidenceRef {
        source: source.to_owned(),
        revision: revision.to_owned(),
        locator: locator.to_owned(),
    }
}

fn assignment(id: &str, content: &str, evidence: Vec<EvidenceRef>) -> MemoryProposal {
    MemoryProposal {
        id: id.to_owned(),
        key: "inventory.a42.custodian".to_owned(),
        kind: MemoryKind::Fact,
        title: "A42 inventory custodian".to_owned(),
        content: content.to_owned(),
        evidence,
    }
}

fn confirm(id: &str, replaces: Option<&str>) -> MemoryChange {
    MemoryChange::Confirm {
        id: id.to_owned(),
        replaces: replaces.map(str::to_owned),
    }
}

fn recall(snapshot: &MemorySnapshot) -> Vec<&str> {
    snapshot
        .recall(RecallQuery {
            text: "A42 inventory custodian",
            kinds: &[],
            limit: 8,
            max_bytes: 4096,
        })
        .iter()
        .map(|record| record.proposal.id.as_str())
        .collect()
}

#[test]
fn proposals_need_confirmation_and_conflicts_preserve_the_review_history() {
    let store = InMemoryStore::default();
    let catalog = MemoryCatalog::new(&store, "warehouse-a").unwrap();
    let original = assignment(
        "first",
        "The A42 inventory custodian is Dock 3.",
        vec![evidence("handover-log", "v1", "entry:17")],
    );
    let draft = catalog
        .apply(
            0,
            &[
                observe("handover-log", Some("v1")),
                MemoryChange::Propose(original.clone()),
            ],
        )
        .unwrap();
    assert_eq!(draft.get("first").unwrap().state, MemoryState::Proposed);
    assert!(recall(&draft).is_empty());
    let retried = catalog
        .apply(draft.revision(), &[MemoryChange::Propose(original)])
        .unwrap();
    assert!(Arc::ptr_eq(&draft, &retried));
    let checked = catalog
        .apply(draft.revision(), &[confirm("first", None)])
        .unwrap();
    assert_eq!(recall(&checked), ["first"]);

    let replacement = assignment(
        "next",
        "The A42 inventory custodian is Dock 9.",
        vec![evidence("handover-log", "v1", "entry:18")],
    );
    let draft = catalog
        .apply(checked.revision(), &[MemoryChange::Propose(replacement)])
        .unwrap();
    assert!(matches!(
        catalog.apply(draft.revision(), &[confirm("next", None)]),
        Err(MemoryError::Conflict { current: Some(id), .. }) if id == "first"
    ));
    assert_eq!(recall(&catalog.snapshot().unwrap()), ["first"]);

    let current = catalog
        .apply(draft.revision(), &[confirm("next", Some("first"))])
        .unwrap();
    assert_eq!(recall(&current), ["next"]);
    let old = current.get("first").unwrap();
    assert_eq!(old.state, MemoryState::Superseded);
    assert_eq!(old.superseded_by.as_deref(), Some("next"));
    assert_eq!(old.proposal.evidence[0].locator, "entry:17");
    assert_eq!(recall(&checked), ["first"]);
    assert!(
        recall(
            &MemoryCatalog::new(&store, "warehouse-b")
                .unwrap()
                .snapshot()
                .unwrap()
        )
        .is_empty()
    );
}

#[test]
fn changed_or_removed_evidence_holds_facts_for_review_until_confirmed_again() {
    let store = InMemoryStore::default();
    let catalog = MemoryCatalog::new(&store, "warehouse-a").unwrap();
    let claim = assignment(
        "assignment",
        "The A42 inventory custodian is Dock 9.",
        vec![
            evidence("handover-log", "v1", "entry:18"),
            evidence("staff-roster", "v5", "shift:3"),
        ],
    );
    let checked = catalog
        .apply(
            0,
            &[
                observe("handover-log", Some("v1")),
                observe("staff-roster", Some("v5")),
                MemoryChange::Propose(claim.clone()),
                confirm("assignment", None),
            ],
        )
        .unwrap();
    let changed = catalog
        .apply(checked.revision(), &[observe("handover-log", Some("v2"))])
        .unwrap();
    assert_eq!(
        changed.current(&claim.key).unwrap().state,
        MemoryState::NeedsReview
    );
    assert!(recall(&changed).is_empty());
    assert!(matches!(
        catalog.apply(changed.revision(), &[confirm("assignment", Some("assignment"))]),
        Err(MemoryError::OutdatedEvidence(reference)) if reference.revision == "v1"
    ));
    let reverted = catalog
        .apply(changed.revision(), &[observe("handover-log", Some("v1"))])
        .unwrap();
    assert!(recall(&reverted).is_empty());
    let confirmed = catalog
        .apply(
            reverted.revision(),
            &[confirm("assignment", Some("assignment"))],
        )
        .unwrap();
    assert_eq!(recall(&confirmed), ["assignment"]);

    let removed = catalog
        .apply(confirmed.revision(), &[observe("staff-roster", None)])
        .unwrap();
    assert_eq!(removed.source_revision("staff-roster"), None);
    assert_eq!(
        removed.get("assignment").unwrap().state,
        MemoryState::NeedsReview
    );
    let retired = catalog
        .apply(
            removed.revision(),
            &[MemoryChange::Withdraw {
                id: "assignment".into(),
            }],
        )
        .unwrap();
    assert_eq!(
        retired.get("assignment").unwrap().state,
        MemoryState::Withdrawn
    );
    assert!(retired.current(&claim.key).is_none());
}

#[test]
fn a_failed_multi_change_batch_publishes_nothing() {
    let store = InMemoryStore::default();
    let catalog = MemoryCatalog::new(&store, "warehouse-a").unwrap();
    let outdated = assignment(
        "outdated",
        "The A42 inventory custodian is Dock 9.",
        vec![evidence("handover-log", "old", "entry:18")],
    );
    assert!(matches!(
        catalog.apply(
            0,
            &[
                observe("handover-log", Some("latest")),
                MemoryChange::Propose(outdated),
                confirm("outdated", None),
            ]
        ),
        Err(MemoryError::OutdatedEvidence(_))
    ));
    let snapshot = catalog.snapshot().unwrap();
    assert_eq!(snapshot.revision(), 0);
    assert!(snapshot.source_revision("handover-log").is_none());
    assert!(snapshot.get("outdated").is_none());
}

#[test]
fn recall_filters_kinds_and_counts_escaped_text_and_evidence_in_its_budget() {
    let store = InMemoryStore::default();
    let catalog = MemoryCatalog::new(&store, "translation-desk").unwrap();
    let make = |id: &str, kind, content: String, evidence| MemoryProposal {
        id: id.to_owned(),
        key: id.to_owned(),
        kind,
        title: "青禾洋运".to_owned(),
        content,
        evidence,
    };
    let long = make(
        "translation",
        MemoryKind::Decision,
        "青禾洋运的名称译为 Qinghe Shipping。".repeat(30),
        vec![evidence("glossary", "v3", r#"sheet:"March"\row:5"#)],
    );
    let small = make(
        "format",
        MemoryKind::Preference,
        "青禾洋运的名称用中文。".to_owned(),
        Vec::new(),
    );
    let state = catalog
        .apply(
            0,
            &[
                observe("glossary", Some("v3")),
                MemoryChange::Propose(long),
                confirm("translation", None),
                MemoryChange::Propose(small),
                confirm("format", None),
            ],
        )
        .unwrap();
    let budget = serde_json::to_vec(&[state.get("format").unwrap()])
        .unwrap()
        .len();
    let query = RecallQuery {
        text: "青禾洋运",
        kinds: &[],
        limit: 8,
        max_bytes: budget,
    };
    let hits = state.recall(query);
    assert_eq!(hits[0].proposal.id, "format");
    assert!(serde_json::to_vec(&hits).unwrap().len() <= budget);
    assert!(
        state
            .recall(RecallQuery {
                max_bytes: budget - 1,
                ..query
            })
            .is_empty()
    );
    let filtered = state.recall(RecallQuery {
        kinds: &[MemoryKind::Decision],
        max_bytes: 4096,
        ..query
    });
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].proposal.id, "translation");
    let public = state.recall_where(
        RecallQuery {
            max_bytes: 4096,
            ..query
        },
        |record| record.proposal.evidence.is_empty(),
    );
    assert_eq!(public.len(), 1);
    assert_eq!(public[0].proposal.id, "format");
}

#[test]
fn a_corrupt_snapshot_cannot_recall_invalid_provenance() {
    let store = InMemoryStore::default();
    let catalog = MemoryCatalog::new(&store, "warehouse-a").unwrap();
    let valid = catalog
        .apply(
            0,
            &[
                observe("handover-log", Some("v1")),
                MemoryChange::Propose(assignment(
                    "first",
                    "A42 Dock 3",
                    vec![evidence("handover-log", "v1", "entry:17")],
                )),
                confirm("first", None),
            ],
        )
        .unwrap();
    let mut encoded = serde_json::to_value(&*valid).unwrap();
    let decoded = serde_json::from_value::<MemorySnapshot>(encoded.clone()).unwrap();
    assert_eq!(recall(&decoded), ["first"]);
    encoded["sources"]["handover-log"] = serde_json::json!("v2");
    assert!(serde_json::from_value::<MemorySnapshot>(encoded).is_err());
}

#[test]
fn a_shared_agent_tool_recalls_current_memory_from_a_frozen_snapshot() {
    use cove_agent::{
        AgentMessage, AgentReply, AgentRole, AgentSession, AgentSkillLibrary, AgentTool,
        AgentToolCall, AgentToolResult, AgentToolbox, JsonSchema,
    };
    use serde_json::json;

    let store = InMemoryStore::default();
    let catalog = MemoryCatalog::new(&store, "warehouse-a").unwrap();
    let confirmed = catalog
        .apply(
            0,
            &[
                observe("handover-log", Some("v1")),
                MemoryChange::Propose(assignment(
                    "first",
                    "A42 Dock 3",
                    vec![evidence("handover-log", "v1", "entry:17")],
                )),
                confirm("first", None),
            ],
        )
        .unwrap();
    let response = |snapshot: Arc<MemorySnapshot>| {
        let toolbox = AgentToolbox::new(
            vec![AgentTool::new(
                "recall_memory",
                "Find confirmed inventory memories",
                JsonSchema::any(),
                true,
            )],
            Arc::new(move |_, args| {
                let records = snapshot.recall(RecallQuery {
                    text: args["query"].as_str().unwrap(),
                    kinds: &[MemoryKind::Fact],
                    limit: 4,
                    max_bytes: 1024,
                });
                Ok(AgentToolResult::encode(&json!(records), false))
            }),
        )
        .unwrap();
        let session = AgentSession::new(
            toolbox,
            &AgentSkillLibrary::default(),
            &[],
            4,
            Arc::new(|messages, _| {
                let last = messages.last().unwrap();
                if last.role != AgentRole::Tool {
                    return Ok(AgentReply::Tools(vec![AgentToolCall {
                        id: "lookup".to_owned(),
                        name: "recall_memory".to_owned(),
                        arguments: json!({"query":"A42 inventory custodian"}),
                    }]));
                }
                let entries: serde_json::Value = serde_json::from_str(&last.text).unwrap();
                let reply = if entries == json!([]) {
                    "Custodian needs review.".to_owned()
                } else {
                    assert_eq!(entries[0]["proposal"]["evidence"][0]["locator"], "entry:17");
                    entries[0]["proposal"]["content"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                };
                Ok(AgentReply::Answer(reply))
            }),
        )
        .unwrap();
        session
            .run(vec![AgentMessage::user("Who has A42?")])
            .unwrap()
            .answer
    };

    let held_for_review = catalog
        .apply(confirmed.revision(), &[observe("handover-log", Some("v2"))])
        .unwrap();
    assert_eq!(response(confirmed), "A42 Dock 3");
    assert_eq!(response(held_for_review), "Custodian needs review.");
}

#[cfg(all(feature = "memory-files", any(unix, windows)))]
#[test]
fn file_backed_actors_persist_history_and_serialize_competing_reviews() {
    use std::sync::Barrier;

    use cove_agent::memory::{FileMemoryStore, MemoryStore};

    let root = tempfile::tempdir().unwrap();
    {
        let store = FileMemoryStore::open(root.path()).unwrap();
        let catalog = MemoryCatalog::new(&store, "warehouse-a").unwrap();
        catalog
            .apply(
                0,
                &[
                    observe("handover-log", Some("v1")),
                    MemoryChange::Propose(assignment(
                        "first",
                        "A42 Dock 3",
                        vec![evidence("handover-log", "v1", "entry:17")],
                    )),
                    confirm("first", None),
                ],
            )
            .unwrap();
        assert!(MemoryCatalog::new(&store, "../other").is_err());
    }
    let store = FileMemoryStore::open(root.path()).unwrap();
    let catalog = MemoryCatalog::new(&store, "warehouse-a").unwrap();
    let snapshot = catalog.snapshot().unwrap();
    assert_eq!(snapshot.revision(), 1);
    assert_eq!(recall(&snapshot), ["first"]);

    // Stage two independent candidate updates based on the same persisted state.
    let staged = |revision: &str| {
        let memory = InMemoryStore::default();
        memory
            .compare_exchange("warehouse-a", 0, snapshot.clone())
            .unwrap();
        MemoryCatalog::new(&memory, "warehouse-a")
            .unwrap()
            .apply(1, &[observe("handover-log", Some(revision))])
            .unwrap()
    };
    let (left, right) = (staged("v2-left"), staged("v2-right"));
    let gate = Barrier::new(2);
    let (a, b) = std::thread::scope(|threads| {
        let write = |next: Arc<MemorySnapshot>| {
            let writer = FileMemoryStore::open(root.path()).unwrap();
            gate.wait();
            writer.compare_exchange("warehouse-a", 1, next)
        };
        let a = threads.spawn(move || write(left));
        let b = threads.spawn(move || write(right));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.err().or_else(|| b.err()),
        Some(MemoryError::Stale {
            expected: 1,
            actual: 2
        })
    ));
    let updated = catalog.snapshot().unwrap();
    assert_eq!(updated.revision(), 2);
    assert!(recall(&updated).is_empty());
    assert_eq!(
        updated.get("first").unwrap().state,
        MemoryState::NeedsReview
    );
}
