//! The canonical diagnostic tool registry.
//!
//! Name, schema and the immediate-read capability live together so the DevTools
//! wire adapter, the MCP toolbox and the handlers cannot drift into separate
//! string lists.
use crate::{AgentError, AgentSkill, AgentSkillLibrary, AgentTool, JsonSchema};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    GetDiagnostics,
    QueryUiNodes,
    QueryUiRecomposition,
    QueryUiState,
    QueryUiMemory,
    CaptureUiTrace,
    GetUiTrace,
    CompareUiTraces,
    ListSkills,
    ReadSkill,
}

impl Tool {
    pub const ALL: [Tool; 10] = [
        Tool::GetDiagnostics,
        Tool::QueryUiNodes,
        Tool::QueryUiRecomposition,
        Tool::QueryUiState,
        Tool::QueryUiMemory,
        Tool::CaptureUiTrace,
        Tool::GetUiTrace,
        Tool::CompareUiTraces,
        Tool::ListSkills,
        Tool::ReadSkill,
    ];

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::GetDiagnostics => "get_diagnostics",
            Self::QueryUiNodes => "query_ui_nodes",
            Self::QueryUiRecomposition => "query_ui_recomposition",
            Self::QueryUiState => "query_ui_state",
            Self::QueryUiMemory => "query_ui_memory",
            Self::CaptureUiTrace => "capture_ui_trace",
            Self::GetUiTrace => "get_ui_trace",
            Self::CompareUiTraces => "compare_ui_traces",
            Self::ListSkills => "list_skills",
            Self::ReadSkill => "read_skill",
        }
    }

    /// Tools the native DevTools wire adapter may execute synchronously in the
    /// host's frame inbox. Trace reads and comparisons stay on the asynchronous
    /// toolbox because they depend on retained capture state.
    pub fn is_immediate_read(self) -> bool {
        !matches!(
            self,
            Self::CaptureUiTrace | Self::GetUiTrace | Self::CompareUiTraces
        )
    }

    pub fn immediate_read_tools() -> Vec<Tool> {
        Self::ALL
            .into_iter()
            .filter(|tool| tool.is_immediate_read())
            .collect()
    }

    /// The contract exposed to MCP clients, embedded agents and DevTools.
    pub fn definition(self) -> AgentTool {
        let text = || JsonSchema::string(Some(1), Some(256));
        let paging = || {
            let mut properties = BTreeMap::new();
            properties.insert(
                "offset".to_string(),
                JsonSchema::integer(Some(0.0), Some(1_000_000.0)),
            );
            properties.insert(
                "limit".to_string(),
                JsonSchema::integer(Some(1.0), Some(100.0)),
            );
            properties
        };
        let object = |properties: BTreeMap<String, JsonSchema>, required: Vec<String>| {
            JsonSchema::object(properties, required)
        };
        let merge = |extra: Vec<(&str, JsonSchema)>, required: Vec<String>| {
            let mut properties = paging();
            for (key, schema) in extra {
                properties.insert(key.to_string(), schema);
            }
            object(properties, required)
        };
        match self {
            Self::GetDiagnostics => AgentTool::new(
                self.name(),
                "Read bounded diagnostic issues with evidence and data availability. Recommendations are hypotheses to verify.",
                object(
                    BTreeMap::from([(
                        "limit".to_string(),
                        JsonSchema::integer(Some(1.0), Some(100.0)),
                    )]),
                    Vec::new(),
                ),
                true,
            ),
            Self::QueryUiMemory => AgentTool::new(
                self.name(),
                "Read logical text/image cache and per-window target memory estimates. Excludes driver pools and external scene textures; this is not process physical footprint. Null means the host has no provider.",
                object(BTreeMap::new(), Vec::new()),
                true,
            ),
            Self::QueryUiNodes => AgentTool::new(
                self.name(),
                "Query live UI nodes by accessible label, value, help, role or source. Text searches also match debug names and view tags; secure values are redacted before matching. Lazy input values are omitted by default: use node_id with include_value=true to read one field, capped at 4096 characters with valueTruncated reported. Secure providers are never read. Returns flat paged results, clipped visible bounds and effective input availability, including modal scopes. Use compact=true for one short record per node (id, role, label, value, state, frame, source). Use visible_only to exclude hidden/offscreen nodes; input_enabled_only also excludes disabled or modal-blocked controls. IDs expire on unmount.",
                merge(
                    vec![
                        ("node_id", text()),
                        ("include_value", JsonSchema::boolean()),
                        ("compact", JsonSchema::boolean()),
                        ("text", text()),
                        ("role", text()),
                        ("source", text()),
                        ("visible_only", JsonSchema::boolean()),
                        ("input_enabled_only", JsonSchema::boolean()),
                    ],
                    Vec::new(),
                ),
                true,
            ),
            Self::QueryUiRecomposition => AgentTool::new(
                self.name(),
                "Query owning components sorted by total inclusive CPU time, including counts and invalidation causes. Child durations overlap; do not sum them as frame time.",
                merge(
                    vec![
                        ("text", text()),
                        (
                            "minimum_count",
                            JsonSchema::integer(Some(0.0), Some(1_000_000.0)),
                        ),
                    ],
                    Vec::new(),
                ),
                true,
            ),
            Self::QueryUiState => AgentTool::new(
                self.name(),
                "List opted-in state metadata. Read summaries only for explicit ids; private state is never exposed. Unmounted ids are reported as missing.",
                merge(
                    vec![
                        ("scope_id", text()),
                        ("ids", JsonSchema::array_of(text(), Some(128))),
                    ],
                    Vec::new(),
                ),
                true,
            ),
            Self::CaptureUiTrace => AgentTool::new(
                self.name(),
                "Capture actual UI CPU spans for 50–5000 ms without forcing frames. Returns a session capture id and summary; idle captures may be empty. Does not measure GPU execution.",
                object(
                    BTreeMap::from([(
                        "duration_ms".to_string(),
                        JsonSchema::integer(Some(50.0), Some(5000.0)),
                    )]),
                    Vec::new(),
                ),
                true,
            ),
            Self::GetUiTrace => AgentTool::new(
                self.name(),
                "Read paged events from a retained capture. At most eight bounded captures are retained in this session.",
                merge(vec![("capture_id", text())], vec!["capture_id".to_string()]),
                true,
            ),
            Self::CompareUiTraces => AgentTool::new(
                self.name(),
                "Compare counts, average and P95 CPU span times by phase for two retained captures. Different workloads and dropped spans limit comparison; changes do not prove causality.",
                object(
                    BTreeMap::from([
                        ("baseline_id".to_string(), text()),
                        ("candidate_id".to_string(), text()),
                    ]),
                    vec!["baseline_id".to_string(), "candidate_id".to_string()],
                ),
                true,
            ),
            Self::ListSkills => AgentTool::new(
                self.name(),
                "List the host's available task guides without loading their bodies.",
                object(BTreeMap::new(), Vec::new()),
                true,
            ),
            Self::ReadSkill => AgentTool::new(
                self.name(),
                "Read a named task guide. Instructions do not grant tools or execute code.",
                object(
                    BTreeMap::from([("name".to_string(), text())]),
                    vec!["name".to_string()],
                ),
                true,
            ),
        }
    }
}

/// Canonical tool contracts used by embedded agents and MCP.
pub struct Tools;

impl Tools {
    pub fn tools() -> Vec<AgentTool> {
        Tool::ALL.into_iter().map(Tool::definition).collect()
    }

    pub fn tool(named: &str) -> Option<Tool> {
        Tool::parse(named)
    }

    pub fn skills() -> AgentSkillLibrary {
        default_skill_library()
    }
}

/// Built-in task guide shipped with the crate.
pub const UI_DIAGNOSTICS_GUIDE: &str = "\
Start with get_diagnostics to discover available data. Use query_ui_nodes to locate the affected component and its compiled source location. Query owner recomposition statistics and explicitly exposed state summaries as needed. Capture a bounded CPU trace around the real operation; an idle capture is not evidence of a successful action. Read capture events and compare equivalent workloads before and after a change. Parent spans include child work; do not sum overlapping spans as frame time. CPU draw and present timing do not measure GPU execution. Cite node, scope, sample or capture IDs with observations. Treat recommendations and correlated timings as hypotheses. Report missing, stale, truncated or dropped data. This guide grants no additional tools or write permissions.";

/// The default library: one built-in guide plus any guide found on disk.
pub fn default_skill_library() -> AgentSkillLibrary {
    let builtin = AgentSkill::new(
        "ui-diagnostics",
        "Investigate UI layout, state and performance with runtime evidence.",
        UI_DIAGNOSTICS_GUIDE,
    );
    match builtin {
        Ok(skill) => AgentSkillLibrary::new(vec![skill]).unwrap_or_default(),
        Err(_) => AgentSkillLibrary::default(),
    }
}

/// Build the portable toolbox that both `AgentSession` and `McpServer` accept.
pub fn toolbox(
    executor: impl Fn(&str, &serde_json::Value) -> Result<crate::AgentToolResult, AgentError>
    + Send
    + Sync
    + 'static,
) -> Result<crate::AgentToolbox, AgentError> {
    crate::AgentToolbox::new(Tools::tools(), std::sync::Arc::new(executor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_unique_name_and_strict_schema() {
        let mut names = std::collections::BTreeSet::new();
        for tool in Tools::tools() {
            assert!(names.insert(tool.name.clone()), "duplicate {}", tool.name);
            assert!(
                tool.schema.is_strict_capability_input(),
                "{} is not a strict input schema",
                tool.name
            );
        }
        assert_eq!(names.len(), Tool::ALL.len());
    }

    #[test]
    fn trace_tools_are_not_immediate_reads() {
        assert!(!Tool::CaptureUiTrace.is_immediate_read());
        assert!(!Tool::GetUiTrace.is_immediate_read());
        assert!(!Tool::CompareUiTraces.is_immediate_read());
        assert_eq!(Tool::immediate_read_tools().len(), 7);
    }

    #[test]
    fn names_round_trip() {
        for tool in Tool::ALL {
            assert_eq!(Tool::parse(tool.name()), Some(tool));
        }
        assert_eq!(Tool::parse("nope"), None);
    }

    #[test]
    fn builtin_guide_is_valid() {
        let library = default_skill_library();
        assert_eq!(library.skills.len(), 1);
        assert!(library.read("ui-diagnostics").is_ok());
    }
}
