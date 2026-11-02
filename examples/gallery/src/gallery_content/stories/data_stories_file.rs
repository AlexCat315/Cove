//! Data story file: tree, lists, swipe, refresh, drag-and-drop and scrollbars.
//!
//! Swift source: `GalleryContent/Stories/Data/DataStories.swift`, which holds
//! thirteen stories.
//!
//! The logic worth pinning is tree search (a match keeps its ancestors visible),
//! the refresh threshold and its re-entrancy guard, and the virtual-stack jump
//! targets. The remaining stories are view compositions whose state is a single
//! binding; they are ported for completeness but carry no algorithm.

use cove_composition::Scope;
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

use cove_core::{Binding, State};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::Selection;
use cove_ui_compose::primitives::data::lists::refresh::RefreshOptions;
use cove_ui_compose::primitives::data::lists::tree::{
    Tree, TreeOptions, TreeSearch, TreeSelection,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::theme::tokens::SemanticColorRef;
use cove_ui_compose::{AnyView, LayoutDebugExt, TupleView, View, ViewExt};

use super::feedback::StorySection;
use crate::gallery_content::catalog::GalleryOptions;

/// The row count the virtual stack materialises from.
pub const VIRTUAL_STACK_COUNT: usize = 10_000;
pub const VIRTUAL_ROW_HEIGHT: f32 = 30.0;
/// The jump targets the three buttons select.
pub const VIRTUAL_FIRST: usize = 0;
pub const VIRTUAL_MIDDLE: usize = 5_000;
pub const VIRTUAL_LAST: usize = 9_999;

/// The pull distance that starts a refresh.
pub const REFRESH_THRESHOLD: f32 = 64.0;
/// The list rows both list stories use.
pub const SWIPE_ROWS: [&str; 5] = [
    "Inbox",
    "Design review",
    "Release notes",
    "Billing",
    "Research",
];
pub const REFRESH_ROWS: [&str; 6] = [
    "Inbox",
    "Design review",
    "Release notes",
    "Billing",
    "Research",
    "Archive",
];

/// A node of the scene hierarchy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneNode {
    pub id: &'static str,
    pub title: &'static str,
    pub children: Vec<SceneNode>,
}

impl SceneNode {
    fn leaf(id: &'static str, title: &'static str) -> Self {
        Self {
            id,
            title,
            children: Vec::new(),
        }
    }

    /// The authored hierarchy.
    pub fn gallery() -> Vec<SceneNode> {
        vec![SceneNode {
            id: "scene",
            title: "Scene",
            children: vec![
                SceneNode {
                    id: "lights",
                    title: "Lights",
                    children: vec![
                        SceneNode::leaf("key", "Key Light"),
                        SceneNode::leaf("fill", "Fill Light"),
                    ],
                },
                SceneNode {
                    id: "objects",
                    title: "Objects",
                    children: vec![SceneNode::leaf("sphere", "Sphere")],
                },
                SceneNode::leaf("camera", "Camera"),
            ],
        }]
    }

    pub fn is_expandable(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Whether `title` matches `query`, case-insensitively.
pub fn node_matches(title: &str, query: &str) -> bool {
    query.is_empty() || title.to_lowercase().contains(&query.to_lowercase())
}

/// The ids to keep visible for `query`, including every ancestor of a match.
///
/// A match deep in the tree is useless if its parents are filtered out, so a node
/// survives when it matches *or* any descendant does.
pub fn visible_ids(nodes: &[SceneNode], query: &str) -> BTreeSet<&'static str> {
    fn walk(nodes: &[SceneNode], query: &str, out: &mut BTreeSet<&'static str>) -> bool {
        let mut any = false;
        for node in nodes {
            let child_hit = walk(&node.children, query, out);
            if child_hit || node_matches(node.title, query) {
                out.insert(node.id);
                any = true;
            }
        }
        any
    }
    let mut out = BTreeSet::new();
    walk(nodes, query, &mut out);
    out
}

/// The ids a "expand all" control should set, which is every expandable node.
pub fn expandable_ids(nodes: &[SceneNode]) -> BTreeSet<&'static str> {
    fn walk(nodes: &[SceneNode], out: &mut BTreeSet<&'static str>) {
        for node in nodes {
            if node.is_expandable() {
                out.insert(node.id);
                walk(&node.children, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(nodes, &mut out);
    out
}

/// `TreeStory`: the scene hierarchy with search and multi-selection.
pub struct TreeStory {
    pub options: GalleryOptions,
    selection: State<Option<String>>,
    expanded: State<BTreeSet<String>>,
    selected_ids: State<BTreeSet<String>>,
    query: State<String>,
}

impl TreeStory {
    pub fn new(options: GalleryOptions) -> Self {
        Self {
            options,
            selection: State::new(Some("camera".to_owned())),
            expanded: State::new(
                ["scene".to_owned(), "lights".to_owned()]
                    .into_iter()
                    .collect(),
            ),
            selected_ids: State::new(["camera".to_owned()].into_iter().collect()),
            query: State::new(String::new()),
        }
    }

    pub fn selection(&self) -> Option<String> {
        self.selection.get()
    }

    pub fn expanded(&self) -> BTreeSet<String> {
        self.expanded.get()
    }

    pub fn selected_ids(&self) -> BTreeSet<String> {
        self.selected_ids.get()
    }

    pub fn query(&self) -> String {
        self.query.get()
    }

    pub fn set_query(&self, query: impl Into<String>) {
        self.query.set(query.into());
    }

    /// Expand every branch. The authored list is exactly the expandable ids.
    pub fn expand_all(&self) {
        self.expanded.set(
            expandable_ids(&SceneNode::gallery())
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>(),
        );
    }

    pub fn collapse_all(&self) {
        self.expanded.set(BTreeSet::new());
    }

    /// The ids still shown for the current query.
    pub fn visible(&self) -> BTreeSet<String> {
        visible_ids(&SceneNode::gallery(), &self.query.get())
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    /// Command-click behaviour: toggle one id in the multi-selection.
    pub fn toggle_multi(&self, id: &str) {
        self.selected_ids.update(|set| {
            if !set.remove(id) {
                set.insert(id.to_owned());
            }
        });
    }

    /// The caption lists the multi-selection in sorted order.
    pub fn caption(&self) -> String {
        let ids: Vec<String> = self.selected_ids.get().into_iter().collect();
        format!("Selected: {}", ids.join(", "))
    }
}

impl View for TreeStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let expanded_state = self.expanded.clone();
        let expanded = Binding::new(
            {
                let state = expanded_state.clone();
                move || state.get().into_iter().collect::<HashSet<String>>()
            },
            move |set: HashSet<String>| expanded_state.set(set.into_iter().collect()),
        );
        let text: Rc<dyn Fn(&SceneNode) -> String> =
            Rc::new(|node: &SceneNode| format!("{} {}", node.title, node.id));
        let options = TreeOptions {
            is_enabled: self.options.is_enabled,
            selection: TreeSelection {
                ids: Selection::multiple(self.selected_ids.binding()),
                expanded: Some(expanded),
                ..TreeSelection::default()
            },
            search: TreeSearch {
                query: self.query.get(),
                text: Some(text),
                ..TreeSearch::default()
            },
            ..TreeOptions::default()
        };
        let tree = AnyView::new(
            Tree::new(
                SceneNode::gallery(),
                |node: &SceneNode| node.id.to_owned(),
                |node: &SceneNode| node.children.clone(),
                |node: &SceneNode, state: RowState| {
                    let selected = state.is_selected;
                    let expands = state.is_expanded;
                    let depth = state.depth;

                    let marker = if node.children.is_empty() {
                        "·"
                    } else if expands {
                        "▾"
                    } else {
                        "▸"
                    };
                    let row = format!("{}{marker} {}", "  ".repeat(depth), node.title);
                    let text = Text::new(row);
                    AnyView::new(if selected {
                        text.semantic_foreground(SemanticColorRef::info())
                    } else {
                        text.semantic_foreground(SemanticColorRef::on_surface())
                    })
                },
            )
            .options(options)
            .frame_size(320.0, 260.0)
            .debug_name("tree-control"),
        );
        let query_state = self.query.clone();
        let search = AnyView::new(
            TextField::new(
                "Filter nodes",
                Binding::new(
                    {
                        let state = query_state.clone();
                        move || TextBuffer::from(state.get())
                    },
                    move |value: TextBuffer| query_state.set(value.string_value()),
                ),
            )
            .frame_width(220.0)
            .debug_name("tree-search"),
        );
        AnyView::new(StorySection::new(
            "Scene hierarchy",
            "Search retains matching ancestors. Use arrows, Home and End; Option/Alt + Right expands all and Option/Alt + Left collapses all. Command/Control-click selects multiple nodes.",
            AnyView::new(TupleView::new(vec![
                search,
                tree,
                AnyView::new(
                    Text::new(self.caption())
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .debug_name("tree-caption"),
                ),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "TreeStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection, expanded, selected_ids, query)
    }
}

/// `VirtualStackStory`: jump targets into a 10,000-row stack.
pub struct VirtualStackStory {
    target: State<usize>,
}

impl VirtualStackStory {
    pub fn new() -> Self {
        Self {
            target: State::new(VIRTUAL_FIRST),
        }
    }

    pub fn target(&self) -> usize {
        self.target.get()
    }

    pub fn scroll_to_first(&self) {
        self.target.set(VIRTUAL_FIRST);
    }

    pub fn scroll_to_middle(&self) {
        self.target.set(VIRTUAL_MIDDLE);
    }

    pub fn scroll_to_last(&self) {
        self.target.set(VIRTUAL_LAST);
    }

    /// The row label, one-based.
    pub fn row_label(index: usize) -> String {
        format!("Record {}", index + 1)
    }
}

impl Default for VirtualStackStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for VirtualStackStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Scroll to index",
            "The viewport materializes rows around the requested index.",
            AnyView::new(
                Text::new(Self::row_label(self.target()))
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "VirtualStackStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; target)
    }
}

/// `SwipeListStory`: rows that reveal up to two trailing actions.
pub struct SwipeListStory {
    selection: State<Option<usize>>,
    log: State<String>,
}

impl SwipeListStory {
    pub fn new() -> Self {
        Self {
            selection: State::new(Some(1)),
            log: State::new("No actions yet".to_owned()),
        }
    }

    pub fn selection(&self) -> Option<usize> {
        self.selection.get()
    }

    pub fn log(&self) -> String {
        self.log.get()
    }

    /// The two trailing actions for a row, in reveal order.
    pub fn actions(index: usize) -> Vec<(&'static str, &'static str, bool)> {
        // (id prefix, title, is_destructive)
        vec![("archive", "Archive", false), ("delete", "Delete", true)]
            .into_iter()
            .map(|(prefix, title, danger)| {
                let _ = index;
                (prefix, title, danger)
            })
            .collect()
    }

    /// Activates a row, which is the pointer and keyboard path.
    pub fn activate(&self, index: usize) -> bool {
        match SWIPE_ROWS.get(index) {
            Some(row) => {
                self.selection.set(Some(index));
                self.log.set(format!("Opened {row}"));
                true
            }
            None => false,
        }
    }

    /// Runs a trailing action.
    pub fn run_action(&self, index: usize, action: &str) -> bool {
        let Some(row) = SWIPE_ROWS.get(index) else {
            return false;
        };
        let verb = match action {
            "archive" => "Archived",
            "delete" => "Deleted",
            _ => return false,
        };
        self.log.set(format!("{verb} {row}"));
        true
    }
}

impl Default for SwipeListStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for SwipeListStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Swipe actions",
            "Touch rows reveal up to two trailing actions. Every row also has an actions button and a right-click menu, so pointer, keyboard and assistive clients can run the same commands without swiping.",
            AnyView::new(
                Text::new(self.log())
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .debug_name("swipe-log"),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "SwipeListStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection, log)
    }
}

/// `RefreshListStory`: a caller-owned refresh that cannot re-enter.
pub struct RefreshListStory {
    is_refreshing: State<bool>,
    refreshes: State<u32>,
}

impl RefreshListStory {
    pub fn new() -> Self {
        Self {
            is_refreshing: State::new(false),
            refreshes: State::new(0),
        }
    }

    pub fn is_refreshing(&self) -> bool {
        self.is_refreshing.get()
    }

    pub fn refreshes(&self) -> u32 {
        self.refreshes.get()
    }

    /// Starts a refresh unless one is already running.
    ///
    /// Returns whether it started; a second call while in flight is ignored so a
    /// fast double-pull cannot queue two refreshes.
    pub fn start_refresh(&self) -> bool {
        if self.is_refreshing.get() {
            return false;
        }
        self.is_refreshing.set(true);
        self.refreshes.update(|value| *value += 1);
        true
    }

    /// The caller clears the flag when its work finishes.
    pub fn finish_refresh(&self) {
        self.is_refreshing.set(false);
    }

    /// The refresh options the list receives.
    pub fn options(&self) -> RefreshOptions {
        RefreshOptions::new(|| {})
            .threshold(REFRESH_THRESHOLD)
            .refreshing(self.is_refreshing.get())
    }
}

impl Default for RefreshListStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for RefreshListStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Pull to refresh",
            "Touch pull past 64pt starts the caller-owned refresh; the indicator holds until the binding clears and freezes under reduced motion. Cmd+R or Ctrl+R is the keyboard path, and the explicit button is the pointer path.",
            AnyView::new(
                Text::new(format!("Refreshes: {}", self.refreshes()))
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                    .debug_name("refresh-count"),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "RefreshListStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; is_refreshing, refreshes)
    }
}

/// `DragAndDropStory`: typed payloads across two lanes.
pub struct DragAndDropStory {
    lane_a: State<Option<String>>,
    lane_b: State<Option<String>>,
}

impl DragAndDropStory {
    pub fn new() -> Self {
        Self {
            lane_a: State::new(None),
            lane_b: State::new(None),
        }
    }

    pub fn lane_a(&self) -> Option<String> {
        self.lane_a.get()
    }

    pub fn lane_b(&self) -> Option<String> {
        self.lane_b.get()
    }

    /// Drops `payload` into a lane.
    ///
    /// Releasing outside every target is not a drop, so this only runs when a
    /// target actually accepted the payload.
    pub fn drop_into_a(&self, payload: impl Into<String>) {
        self.lane_a.set(Some(payload.into()));
    }

    pub fn drop_into_b(&self, payload: impl Into<String>) {
        self.lane_b.set(Some(payload.into()));
    }

    /// The authoring rule: only a payload of the expected kind is accepted.
    pub fn accepts(payload_kind: &str, accepted_kind: &str) -> bool {
        payload_kind == accepted_kind
    }
}

impl Default for DragAndDropStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for DragAndDropStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Typed payloads",
            "Drag a tag into either lane. The preview is portal-rendered, the target highlights while hovered, and only a matching payload calls its drop closure. Escape is not required for a cancelled drag: releasing outside every target drops nothing.",
            AnyView::new(
                Text::new(format!(
                    "A: {} · B: {}",
                    self.lane_a().unwrap_or_else(|| "empty".to_owned()),
                    self.lane_b().unwrap_or_else(|| "empty".to_owned())
                ))
                .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "DragAndDropStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; lane_a, lane_b)
    }
}

/// `SectionedListStory`: three sections with independent headers.
pub struct SectionedListStory {
    selection: State<Option<usize>>,
}

impl SectionedListStory {
    pub fn new() -> Self {
        Self {
            selection: State::new(Some(0)),
        }
    }

    pub fn selection(&self) -> Option<usize> {
        self.selection.get()
    }

    /// The section headers, in order.
    pub fn sections(&self) -> [&'static str; 3] {
        ["Today", "Yesterday", "Earlier"]
    }
}

impl Default for SectionedListStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for SectionedListStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(StorySection::new(
            "Sectioned list",
            "Headers stay with their first row while scrolling.",
            AnyView::new(
                Text::new(format!("{} sections", self.sections().len()))
                    .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
            ),
        ))
    }

    fn tag(&self) -> &'static str {
        "SectionedListStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; selection)
    }
}

/// The five column kinds the chart story plots.
pub const CHART_MARKS: [&str; 5] = ["area", "line", "bar", "point", "rule"];
