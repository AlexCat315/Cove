//! The canonical diagnostic tool registry.
//!
//! Name, schema and the immediate-read capability live together so the DevTools
//! wire adapter, the MCP toolbox and the handlers cannot drift into separate
//! string lists.
use crate::{AgentError, AgentSkill, AgentSkillLibrary, AgentTool, JsonSchema, JsonSchemaScalar};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tool {
    GetDiagnostics,
    QueryUiNodes,
    QueryUiRecomposition,
    QueryUiLayout,
    QueryUiComponentActivity,
    QueryUiSemantics,
    QueryUiState,
    QueryUiMemory,
    QueryUiNetwork,
    CaptureUiTrace,
    GetUiTrace,
    CompareUiTraces,
    StartUiProfile,
    StopUiProfile,
    GetUiProfile,
    QueryUiHotspots,
    StartUiAllocationProfile,
    StopUiAllocationProfile,
    QueryUiAllocationSites,
    ListExtensions,
    SetExtensionEnabled,
    ListSkills,
    ReadSkill,
}

impl Tool {
    pub const ALL: [Tool; 23] = [
        Tool::GetDiagnostics,
        Tool::QueryUiNodes,
        Tool::QueryUiRecomposition,
        Tool::QueryUiLayout,
        Tool::QueryUiComponentActivity,
        Tool::QueryUiSemantics,
        Tool::QueryUiState,
        Tool::QueryUiMemory,
        Tool::QueryUiNetwork,
        Tool::CaptureUiTrace,
        Tool::GetUiTrace,
        Tool::CompareUiTraces,
        Tool::StartUiProfile,
        Tool::StopUiProfile,
        Tool::GetUiProfile,
        Tool::QueryUiHotspots,
        Tool::StartUiAllocationProfile,
        Tool::StopUiAllocationProfile,
        Tool::QueryUiAllocationSites,
        Tool::ListExtensions,
        Tool::SetExtensionEnabled,
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
            Self::QueryUiLayout => "query_ui_layout",
            Self::QueryUiComponentActivity => "query_ui_component_activity",
            Self::QueryUiSemantics => "query_ui_semantics",
            Self::QueryUiState => "query_ui_state",
            Self::QueryUiMemory => "query_ui_memory",
            Self::QueryUiNetwork => "query_ui_network",
            Self::CaptureUiTrace => "capture_ui_trace",
            Self::GetUiTrace => "get_ui_trace",
            Self::CompareUiTraces => "compare_ui_traces",
            Self::StartUiProfile => "start_ui_profile",
            Self::StopUiProfile => "stop_ui_profile",
            Self::GetUiProfile => "get_ui_profile",
            Self::QueryUiHotspots => "query_ui_hotspots",
            Self::StartUiAllocationProfile => "start_ui_allocation_profile",
            Self::StopUiAllocationProfile => "stop_ui_allocation_profile",
            Self::QueryUiAllocationSites => "query_ui_allocation_sites",
            Self::ListExtensions => "list_extensions",
            Self::SetExtensionEnabled => "set_extension_enabled",
            Self::ListSkills => "list_skills",
            Self::ReadSkill => "read_skill",
        }
    }

    /// Tools the native DevTools wire adapter may execute synchronously in the
    /// host's frame inbox. Trace reads, trace comparisons and the UI profile
    /// commands stay on the asynchronous toolbox: they depend on retained
    /// capture or sampler state.
    pub fn is_immediate_read(self) -> bool {
        !matches!(
            self,
            Self::CaptureUiTrace
                | Self::GetUiTrace
                | Self::CompareUiTraces
                | Self::StartUiProfile
                | Self::StopUiProfile
                | Self::GetUiProfile
                | Self::QueryUiHotspots
                | Self::StartUiAllocationProfile
                | Self::StopUiAllocationProfile
                | Self::QueryUiAllocationSites
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
            Self::QueryUiNetwork => AgentTool::new(
                self.name(),
                "Read bounded in-process HTTP activity recorded by cove-http: method, URL, status, timings, headers (credentials redacted) and truncated bodies.",
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
            Self::QueryUiLayout => AgentTool::new(
                self.name(),
                "Read one live node's actual layout inputs, resolved size, layout parent and first 32 layout children.",
                object(
                    BTreeMap::from([("node_id".into(), text())]),
                    vec!["node_id".into()],
                ),
                true,
            ),
            Self::QueryUiComponentActivity => AgentTool::new(
                self.name(),
                "Rank live components by measured rebuilds, layout computations or paints. Counts are cumulative for the currently mounted nodes.",
                merge(
                    vec![
                        ("text", text()),
                        (
                            "sort_by",
                            JsonSchema {
                                allowed_values: ["build", "layout", "paint"]
                                    .map(|name| JsonSchemaScalar::String(name.into()))
                                    .into(),
                                ..JsonSchema::string(None, None)
                            },
                        ),
                    ],
                    Vec::new(),
                ),
                true,
            ),
            Self::QueryUiSemantics => AgentTool::new(
                self.name(),
                "Page through the Cove runtime's own semantics tree. Values from text fields are excluded.",
                object(paging(), Vec::new()),
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
            Self::StartUiProfile => AgentTool::new(
                self.name(),
                "Sample the selected runtime's UI thread with the in-process CPU sampler. Sampling stops by itself after duration_ms, bounded to 10s; the symbolization runs on a background thread. Supported on 64-bit macOS, iOS, Windows, Linux and Android. Other targets report unavailable.",
                object(
                    BTreeMap::from([
                        (
                            "hz".to_string(),
                            JsonSchema::integer(Some(1.0), Some(1_000.0)),
                        ),
                        (
                            "duration_ms".to_string(),
                            JsonSchema::integer(Some(1.0), Some(10_000.0)),
                        ),
                    ]),
                    vec!["hz".to_string(), "duration_ms".to_string()],
                ),
                false,
            ),
            Self::StopUiProfile => AgentTool::new(
                self.name(),
                "Stop the active UI profile, join the sampler and return the symbolized profile: aggregated stacks, retained and dropped sample counts, and the bound. Supported on 64-bit macOS, iOS, Windows, Linux and Android.",
                object(BTreeMap::new(), Vec::new()),
                false,
            ),
            Self::GetUiProfile => AgentTool::new(
                self.name(),
                "Read the active or most recent UI profile. Stacks are aggregated, never one record per sample, and at most 24 are returned. While sampling, counters are live and stacks are empty. Supported on 64-bit macOS, iOS, Windows, Linux and Android.",
                object(BTreeMap::new(), Vec::new()),
                true,
            ),
            Self::QueryUiHotspots => AgentTool::new(
                self.name(),
                "Read aggregated self and inclusive hotspots of the retained UI profile, by function and source line. Self time is exclusive; inclusive time counts a frame once per sample. Times accumulate the measured gap between retained samples. Supported on 64-bit macOS, iOS, Windows, Linux and Android.",
                object(
                    BTreeMap::from([(
                        "limit".to_string(),
                        JsonSchema::integer(Some(1.0), Some(200.0)),
                    )]),
                    Vec::new(),
                ),
                true,
            ),
            Self::StartUiAllocationProfile => AgentTool::new(
                self.name(),
                "Start sampling allocation sites in the selected runtime's process. rate is the 1-in-N divisor: every rate-th fresh allocation through the tracking allocator captures a bounded caller stack. Counters are cumulative sampled allocations, not live heap, and no object types, retaining paths or leaks are inferred. A start replaces any previous session. Requires a runtime that advertises allocation_sites; unsupported platforms return unavailable with the reason.",
                object(
                    BTreeMap::from([(
                        "rate".to_string(),
                        JsonSchema::integer(Some(1.0), Some(1_000_000.0)),
                    )]),
                    vec!["rate".to_string()],
                ),
                false,
            ),
            Self::StopUiAllocationProfile => AgentTool::new(
                self.name(),
                "Stop allocation-site sampling and return the window's bounded, symbolized sites with sampled bytes and blocks, attempted/retained/dropped samples, capture failures and truncation flags. Addresses are real sampled return addresses; function, file and line stay absent when the platform cannot symbolize. These are cumulative sampled allocations, not live heap.",
                object(BTreeMap::new(), Vec::new()),
                false,
            ),
            Self::QueryUiAllocationSites => AgentTool::new(
                self.name(),
                "Read the live allocation-site sampling session's sites, or the last stopped report. limit caps returned sites (default 20, max 64); sitesTruncated reports the cut. Counters are cumulative sampled allocations, not live heap. Idle means this probe session never sampled.",
                object(
                    BTreeMap::from([(
                        "limit".to_string(),
                        JsonSchema::integer(Some(1.0), Some(64.0)),
                    )]),
                    Vec::new(),
                ),
                true,
            ),
            Self::ListExtensions => AgentTool::new(
                self.name(),
                "List the DevTools extensions this runtime registered: id, name, description, version, entry point and whether the panel is enabled. Packages opt in by registering an extension; an empty list means none are active.",
                object(BTreeMap::new(), Vec::new()),
                true,
            ),
            Self::SetExtensionEnabled => AgentTool::new(
                self.name(),
                "Enable or disable a DevTools extension's panel. The change persists for this session; the client reloads the extension list to reflect it. False means the id was not registered.",
                object(
                    BTreeMap::from([
                        ("id".to_string(), text()),
                        ("enabled".to_string(), JsonSchema::boolean()),
                    ]),
                    vec!["id".to_string(), "enabled".to_string()],
                ),
                false,
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
Start with get_diagnostics to discover available data. Use query_ui_nodes to locate the affected component and its compiled source location. Query owner recomposition statistics and explicitly exposed state summaries as needed. Capture a bounded CPU trace around the real operation; an idle capture is not evidence of a successful action. On targets advertising cpu_profile, sample the UI thread with start_ui_profile, then read get_ui_profile and query_ui_hotspots to attribute its time to function and source line. Read capture events and compare equivalent workloads before and after a change. Parent spans include child work; do not sum overlapping spans as frame time. CPU draw and present timing do not measure GPU execution. Cite node, scope, sample or capture IDs with observations. Treat recommendations and correlated timings as hypotheses. Report missing, stale, truncated or dropped data. This guide grants no additional tools or write permissions.";

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
    fn asynchronous_tools_are_not_immediate_reads() {
        for tool in [
            Tool::CaptureUiTrace,
            Tool::GetUiTrace,
            Tool::CompareUiTraces,
            Tool::StartUiProfile,
            Tool::StopUiProfile,
            Tool::GetUiProfile,
            Tool::QueryUiHotspots,
            Tool::StartUiAllocationProfile,
            Tool::StopUiAllocationProfile,
            Tool::QueryUiAllocationSites,
        ] {
            assert!(!tool.is_immediate_read(), "{}", tool.name());
        }
        assert_eq!(Tool::immediate_read_tools().len(), 13);
    }

    #[test]
    fn allocation_site_tools_are_bounded_and_name_their_basis() {
        let start = Tool::StartUiAllocationProfile.definition();
        assert!(!start.read_only, "starting sampling changes runtime state");
        assert_eq!(start.schema.required, vec!["rate".to_string()]);
        assert_eq!(start.schema.properties["rate"].maximum, Some(1_000_000.0));
        assert!(
            start.description.contains("not live heap"),
            "{} names what the counters measure",
            start.name
        );

        assert!(!Tool::StopUiAllocationProfile.definition().read_only);

        let query = Tool::QueryUiAllocationSites.definition();
        assert!(query.read_only);
        let limit = &query.schema.properties["limit"];
        let maximum = limit
            .maximum
            .or_else(|| limit.one_of.iter().find_map(|candidate| candidate.maximum));
        assert_eq!(maximum, Some(64.0));
        assert!(
            query.description.contains("not live heap"),
            "{} names what the counters measure",
            query.name
        );
    }

    #[test]
    fn ui_profile_tools_declare_their_platform_and_mutation() {
        let start = Tool::StartUiProfile.definition();
        assert!(!start.read_only);
        assert_eq!(
            start.schema.required,
            vec!["duration_ms".to_string(), "hz".to_string()]
        );
        assert!(!Tool::StopUiProfile.definition().read_only);
        assert!(Tool::GetUiProfile.definition().read_only);
        assert!(Tool::QueryUiHotspots.definition().read_only);
        for tool in [
            Tool::StartUiProfile,
            Tool::StopUiProfile,
            Tool::GetUiProfile,
            Tool::QueryUiHotspots,
        ] {
            assert!(
                tool.definition().description.contains("macOS"),
                "{} names the platform",
                tool.name()
            );
        }
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
