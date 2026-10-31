//! Translation of `Primitives/Controls/Fields/Combobox/{RemoteSearch,Results}.swift`
//! (the model half: request generations, cancellation and phase selection).
//!
//! Remote results for a combobox: the component debounces typing, cancels
//! superseded requests, and shows loading, failure (with retry) and empty states.
//! The caller's data layer only maps a query to options.
//!
//! Behaviour checklist:
//! - a query shorter than the minimum publishes `TooShort` without requesting
//! - repeating the current query while loading or loaded does not re-request,
//!   unless forced
//! - only the newest generation may publish, so a slow response cannot overwrite
//!   a later query's results
//! - cancelling a loading request returns to `Idle` and clears the query
//! - cancelling keeps loaded results for the next open
//! - duplicate result values are dropped, keeping the first
//! - the results area shows the previous list dimmed while refreshing

use std::collections::HashSet;

/// The state of a combobox's remote search.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ComboboxSearchPhase {
    #[default]
    Idle,
    /// The query is shorter than the configured minimum.
    TooShort(usize),
    Loading,
    Loaded,
    Failed(String),
}

impl ComboboxSearchPhase {
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// A searchable option list with request bookkeeping.
///
/// The generation counter is the whole point: results are published only if the
/// generation still matches, so a slow request for an old query cannot land on
/// top of a newer one.
#[derive(Clone, Debug, Default)]
pub struct ComboboxSearchSession {
    phase: ComboboxSearchPhase,
    results: Vec<String>,
    query: Option<String>,
    generation: u64,
}

impl ComboboxSearchSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn phase(&self) -> &ComboboxSearchPhase {
        &self.phase
    }

    pub fn results(&self) -> &[String] {
        &self.results
    }

    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Decides whether a request should start at all.
    ///
    /// Repeating the current query is skipped unless the last attempt failed or
    /// the caller forces a retry — otherwise every keystroke that leaves the
    /// text unchanged would refetch.
    pub fn should_request(&self, query: &str, force: bool) -> bool {
        if force {
            return true;
        }
        if self.query.as_deref() != Some(query) {
            return true;
        }
        self.phase == ComboboxSearchPhase::Idle || self.phase.is_failure()
    }

    /// Begins a request for `query`.
    ///
    /// Returns `None` when the request was skipped or the query is too short; the
    /// session has already published the resulting phase in that case.
    pub fn begin(&mut self, query: &str, minimum_query_length: usize, force: bool) -> Option<u64> {
        if !self.should_request(query, force) {
            return None;
        }
        // A new query always supersedes the previous one.
        self.generation = self.generation.wrapping_add(1);
        self.query = Some(query.to_owned());
        let trimmed = query.trim();
        // `minimumQueryLength` is counted in characters, not bytes.
        if trimmed.chars().count() < minimum_query_length {
            self.results.clear();
            self.phase = ComboboxSearchPhase::TooShort(minimum_query_length);
            return None;
        }
        self.phase = ComboboxSearchPhase::Loading;
        Some(self.generation)
    }

    /// Publishes results, but only for the newest generation.
    ///
    /// Returns whether the delivery was accepted.
    pub fn deliver(&mut self, generation: u64, mut options: Vec<String>) -> bool {
        if generation != self.generation {
            return false;
        }
        // Duplicate values are dropped, keeping the first occurrence.
        let mut seen = HashSet::new();
        options.retain(|option| seen.insert(option.clone()));
        self.results = options;
        self.phase = ComboboxSearchPhase::Loaded;
        true
    }

    /// Publishes a failure, but only for the newest generation.
    pub fn fail(&mut self, generation: u64, message: impl Into<String>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.phase = ComboboxSearchPhase::Failed(message.into());
        true
    }

    /// Cancels the pending request.
    ///
    /// A loading request returns to `Idle` and forgets its query so the next open
    /// re-requests; already-loaded results are kept for that open.
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.phase.is_loading() {
            self.query = None;
            self.phase = ComboboxSearchPhase::Idle;
        }
    }
}

/// What the results area below the search field shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboboxResultsPresentation {
    /// A spinner, before the first results arrive.
    Searching,
    /// A failure message with a retry affordance.
    Failed(String),
    /// A prompt asking for more characters.
    TooShort(usize),
    /// The configured empty text.
    Empty(String),
    /// The list, dimmed while a refresh is in flight.
    List { dimmed: bool },
}

/// Chooses what the results area renders.
///
/// A refresh keeps the previous results visible but dimmed, so typing never makes
/// the popover jump between states.
pub fn results_presentation(
    phase: &ComboboxSearchPhase,
    match_count: usize,
    empty_text: &str,
) -> ComboboxResultsPresentation {
    match phase {
        // The spinner only shows before there is anything to show.
        ComboboxSearchPhase::Loading if match_count == 0 => ComboboxResultsPresentation::Searching,
        ComboboxSearchPhase::Failed(message) => {
            ComboboxResultsPresentation::Failed(message.clone())
        }
        ComboboxSearchPhase::TooShort(count) => ComboboxResultsPresentation::TooShort(*count),
        _ if match_count == 0 => ComboboxResultsPresentation::Empty(empty_text.to_owned()),
        _ => ComboboxResultsPresentation::List {
            dimmed: phase.is_loading(),
        },
    }
}

/// The prompt text for a too-short query, pluralised.
pub fn too_short_prompt(count: usize) -> String {
    format!(
        "Type at least {count} character{} to search",
        if count == 1 { "" } else { "s" }
    )
}

/// The default failure message when the error carries none.
pub const DEFAULT_FAILURE_MESSAGE: &str = "Couldn’t load results";
