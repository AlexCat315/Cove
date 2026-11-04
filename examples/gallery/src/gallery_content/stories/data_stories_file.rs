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
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use cove_ui_compose::primitives::controls::{Button, Toggle};
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::Selection;
use cove_ui_compose::primitives::data::lists::refresh::RefreshOptions;
use cove_ui_compose::primitives::data::lists::swipe_list::SwipeList;
use cove_ui_compose::primitives::data::lists::tree::{
    Tree, TreeCheck, TreeEvents, TreeExpansion, TreeLazy, TreeNodeKey, TreeOptions, TreeSearch,
    TreeSelection, TreeSlots,
};
use cove_ui_compose::primitives::data::{List, ListSection, SectionedList, SwipeAction};
use cove_ui_compose::primitives::feedback::StatusTone;
use cove_ui_compose::primitives::interaction::{DragScope, Draggable, DropTarget};
use cove_ui_compose::primitives::layout::virtualization::FixedVirtualStack;
use cove_ui_compose::primitives::media::{Icon, ui_icon_catalog};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::TextField;
use cove_ui_compose::primitives::text::text_field::text_field_options::TextFieldBehavior;
use cove_ui_compose::theme::SemanticFontRef;
use cove_ui_compose::theme::styles::buttons::{ButtonStyleViewExt, SecondaryButtonStyle};
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticIconSize};
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

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

/// `TreeStory`: the scene hierarchy with search, checks, selection, accordion,
/// strict checks, a lazy branch and the event log.
pub struct TreeStory {
    pub options: GalleryOptions,
    selection: State<Option<String>>,
    expanded: State<BTreeSet<String>>,
    selected_ids: State<BTreeSet<String>>,
    checked: State<BTreeSet<String>>,
    strict: State<bool>,
    accordion: State<bool>,
    lazy: State<bool>,
    loaded: State<BTreeSet<String>>,
    status: State<String>,
    query: State<String>,
}

/// The branch the lazy-loading demo gates until it is expanded.
pub const LAZY_TREE_BRANCH: &str = "objects";

fn collect_ids(nodes: &[SceneNode], out: &mut BTreeSet<String>) {
    for node in nodes {
        out.insert(node.id.to_owned());
        collect_ids(&node.children, out);
    }
}

fn gallery_ids() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    collect_ids(&SceneNode::gallery(), &mut out);
    out
}

fn boolean_binding(state: &State<bool>) -> Binding<bool> {
    let get = state.clone();
    let set = state.clone();
    Binding::new(move || get.get(), move |value| set.set(value))
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
            checked: State::new(["camera".to_owned()].into_iter().collect()),
            strict: State::new(false),
            accordion: State::new(false),
            lazy: State::new(false),
            loaded: State::new(BTreeSet::new()),
            status: State::new("Ready".to_owned()),
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

    pub fn checked_ids(&self) -> BTreeSet<String> {
        self.checked.get()
    }

    pub fn strict(&self) -> bool {
        self.strict.get()
    }

    pub fn accordion(&self) -> bool {
        self.accordion.get()
    }

    pub fn lazy(&self) -> bool {
        self.lazy.get()
    }

    pub fn loaded(&self) -> BTreeSet<String> {
        self.loaded.get()
    }

    pub fn status(&self) -> String {
        self.status.get()
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

    pub fn check_all(&self) {
        self.checked.set(gallery_ids());
    }

    pub fn clear_checked(&self) {
        self.checked.set(BTreeSet::new());
    }

    pub fn toggle_strict(&self) {
        self.strict.update(|value| *value = !*value);
    }

    pub fn toggle_accordion(&self) {
        self.accordion.update(|value| *value = !*value);
    }

    /// Turning lazy mode on resets the branch so the next expansion asks the
    /// host for its children again.
    pub fn toggle_lazy(&self) {
        self.lazy.update(|value| *value = !*value);
        self.loaded.update(|set| {
            set.remove(LAZY_TREE_BRANCH);
        });
        self.expanded.update(|set| {
            set.remove(LAZY_TREE_BRANCH);
        });
    }

    /// Host side of `load_data`: record the branch and report the load.
    pub fn mark_loaded(&self, id: &str) {
        self.loaded.update(|set| {
            set.insert(id.to_owned());
        });
        self.status.set(format!("Loaded {id}"));
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

    /// The caption lists the multi-selection and the checked count.
    pub fn caption(&self) -> String {
        let ids: Vec<String> = self.selected_ids.get().into_iter().collect();
        format!(
            "Selected: {} · Checked: {}",
            ids.join(", "),
            self.checked.get().len()
        )
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
        let checked_state = self.checked.clone();
        let checked = Binding::new(
            {
                let state = checked_state.clone();
                move || state.get().into_iter().collect::<HashSet<String>>()
            },
            move |set: HashSet<String>| checked_state.set(set.into_iter().collect()),
        );
        let text: Rc<dyn Fn(&SceneNode) -> String> =
            Rc::new(|node: &SceneNode| format!("{} {}", node.title, node.id));

        let lazy_enabled = self.lazy.get();
        let loaded_children = self.loaded.clone();
        let children = move |node: &SceneNode| {
            if lazy_enabled
                && node.id == LAZY_TREE_BRANCH
                && !loaded_children.get().contains(LAZY_TREE_BRANCH)
            {
                Vec::new()
            } else {
                node.children.clone()
            }
        };

        let load_status = self.status.clone();
        let load_loaded = self.loaded.clone();
        let load_data: Rc<dyn Fn(&SceneNode)> = Rc::new(move |node: &SceneNode| {
            load_loaded.update(|set| {
                set.insert(node.id.to_owned());
            });
            load_status.set(format!("Loaded {}", node.title));
        });

        let icon_slot: Rc<dyn Fn(&SceneNode) -> AnyView> = Rc::new(|node: &SceneNode| {
            let icon = if node.children.is_empty() {
                ui_icon_catalog::file::file_text()
            } else {
                ui_icon_catalog::file::folder()
            };
            AnyView::new(Icon::with_semantic(
                icon,
                14.0,
                SemanticColorRef::on_surface_muted(),
            ))
        });

        let select_status = self.status.clone();
        let on_select: Rc<dyn Fn(SceneNode)> = Rc::new(move |node: SceneNode| {
            select_status.set(format!("Selected {}", node.title));
        });
        let expand_status = self.status.clone();
        let on_expand: Rc<
            dyn Fn(&SceneNode, bool, &HashSet<TreeNodeKey<String>>),
        > = Rc::new(
            move |node: &SceneNode, expanded: bool, _keys: &HashSet<TreeNodeKey<String>>| {
                expand_status.set(format!(
                    "{} {}",
                    node.title,
                    if expanded { "expanded" } else { "collapsed" }
                ));
            },
        );
        let check_status = self.status.clone();
        let on_check: Rc<
            dyn Fn(&SceneNode, bool, &HashSet<TreeNodeKey<String>>, &HashSet<TreeNodeKey<String>>),
        > = Rc::new(
            move |node: &SceneNode,
                  checked: bool,
                  checked_keys: &HashSet<TreeNodeKey<String>>,
                  half_keys: &HashSet<TreeNodeKey<String>>| {
                check_status.set(format!(
                    "{} {} · {} checked · {} half",
                    node.title,
                    if checked { "checked" } else { "unchecked" },
                    checked_keys.len(),
                    half_keys.len()
                ));
            },
        );

        let options = TreeOptions {
            is_enabled: self.options.is_enabled,
            selection: TreeSelection {
                ids: Selection::multiple(self.selected_ids.binding()),
                expanded: Some(expanded),
                ..TreeSelection::default()
            },
            check: TreeCheck {
                is_enabled: self.options.is_enabled,
                checked: Some(checked),
                check_strictly: self.strict.get(),
                on_change: Some(on_check),
                ..TreeCheck::default()
            },
            expansion: TreeExpansion {
                accordion: self.accordion.get(),
                ..TreeExpansion::default()
            },
            lazy: TreeLazy {
                is_enabled: self.options.is_enabled && lazy_enabled,
                is_leaf: Some(Rc::new(|node: &SceneNode| {
                    node.id != LAZY_TREE_BRANCH && node.children.is_empty()
                })),
                load_data: Some(load_data),
                ..TreeLazy::default()
            },
            slots: TreeSlots {
                icon: Some(icon_slot),
                ..TreeSlots::default()
            },
            search: TreeSearch {
                query: self.query.get(),
                text: Some(text),
                ..TreeSearch::default()
            },
            events: TreeEvents {
                on_select: Some(on_select),
                on_expand_change: Some(on_expand),
                ..TreeEvents::default()
            },
            ..TreeOptions::default()
        };
        let tree = AnyView::new(
            Tree::new(
                SceneNode::gallery(),
                |node: &SceneNode| node.id.to_owned(),
                children,
                |node: &SceneNode, _: RowState| {
                    // The tree already paints indentation, disclosure and the
                    // selected band; repeating them in the label double-drew
                    // markers and clipped the text inside a narrow band.
                    AnyView::new(Text::new(node.title))
                },
            )
            .options(options)
            .frame_size(360.0, 280.0)
            .debug_name("tree-control"),
        );

        let query_state = self.query.clone();
        let search = AnyView::new(
            TextField::new(
                "Search nodes…",
                Binding::new(
                    {
                        let state = query_state.clone();
                        move || TextBuffer::from(state.get())
                    },
                    move |value: TextBuffer| query_state.set(value.string_value()),
                ),
            )
            .behavior(TextFieldBehavior {
                clearable: true,
                disabled: !self.options.is_enabled,
                ..TextFieldBehavior::default()
            })
            .frame_width(260.0)
            .debug_name("tree-search"),
        );
        let expand = {
            let expanded = self.expanded.clone();
            let enabled = self.options.is_enabled;
            AnyView::new(
                Button::title("Expand all", move || {
                    expanded.set(
                        expandable_ids(&SceneNode::gallery())
                            .into_iter()
                            .map(str::to_owned)
                            .collect::<BTreeSet<_>>(),
                    );
                })
                .enabled(enabled)
                .button_style(SecondaryButtonStyle),
            )
        };
        let collapse = {
            let expanded = self.expanded.clone();
            let enabled = self.options.is_enabled;
            AnyView::new(
                Button::title("Collapse all", move || expanded.set(BTreeSet::new()))
                    .enabled(enabled)
                    .button_style(SecondaryButtonStyle),
            )
        };
        let check_all = {
            let checked = self.checked.clone();
            let enabled = self.options.is_enabled;
            AnyView::new(
                Button::title("Check all", move || checked.set(gallery_ids()))
                    .enabled(enabled)
                    .button_style(SecondaryButtonStyle),
            )
        };
        let clear_checks = {
            let checked = self.checked.clone();
            let enabled = self.options.is_enabled;
            AnyView::new(
                Button::title("Clear checks", move || checked.set(BTreeSet::new()))
                    .enabled(enabled)
                    .button_style(SecondaryButtonStyle),
            )
        };

        let strict = AnyView::new(Toggle::labeled(
            "Strict check",
            boolean_binding(&self.strict),
        ));
        let accordion = AnyView::new(Toggle::labeled(
            "Accordion",
            boolean_binding(&self.accordion),
        ));
        let lazy_state = self.lazy.clone();
        let lazy_loaded = self.loaded.clone();
        let lazy_expanded = self.expanded.clone();
        let lazy = AnyView::new(Toggle::labeled(
            "Lazy Objects",
            Binding::new(
                {
                    let state = lazy_state.clone();
                    move || state.get()
                },
                move |value: bool| {
                    lazy_state.set(value);
                    lazy_loaded.set(BTreeSet::new());
                    lazy_expanded.update(|set| {
                        set.remove(LAZY_TREE_BRANCH);
                    });
                },
            ),
        ));

        let button_row = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::Wrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                search,
                expand,
                collapse,
                check_all,
                clear_checks,
            ])),
        ));
        let toggle_row = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(vec![strict, accordion, lazy])),
        ));
        let toolbar = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![button_row, toggle_row])),
        ));
        AnyView::new(StorySection::new(
            "Scene hierarchy",
            "Search retains matching ancestors. Arrows, Home and End move; Option/Alt + Right expands all and Option/Alt + Left collapses all. Command/Control-click selects multiple nodes. Checking cascades unless Strict check is on; Accordion keeps one branch open; Lazy Objects asks the host for children on first expansion.",
            AnyView::new(TupleView::new(vec![
                toolbar,
                tree,
                AnyView::new(
                    Text::new(self.caption())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .debug_name("tree-caption"),
                ),
                AnyView::new(
                    Text::new(self.status())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::accent())
                        .debug_name("tree-status"),
                ),
            ])),
        ))
    }

    fn tag(&self) -> &'static str {
        "TreeStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(
            self;
            selection,
            expanded,
            selected_ids,
            checked,
            strict,
            accordion,
            lazy,
            loaded,
            status,
            query
        )
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

impl VirtualStackStory {
    fn handle(&self) -> Self {
        Self {
            target: self.target.clone(),
        }
    }
}

impl View for VirtualStackStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let ids: Vec<usize> = (0..VIRTUAL_STACK_COUNT).collect();
        let stack = AnyView::new(
            FixedVirtualStack::new(ids, VIRTUAL_ROW_HEIGHT, |index: &usize| {
                AnyView::new(Text::new(Self::row_label(*index)))
            })
            .scroll_to_index(self.target())
            .frame_height(220.0)
            .debug_name("virtual-stack"),
        );
        let first = self.handle();
        let middle = self.handle();
        let last = self.handle();
        let controls = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Row,
            cove_core::scene::Align::Center,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(Button::title("First", move || first.scroll_to_first())),
                AnyView::new(Button::title("Middle", move || middle.scroll_to_middle())),
                AnyView::new(Button::title("Last", move || last.scroll_to_last())),
                AnyView::new(
                    Text::new(Self::row_label(self.target()))
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                ),
            ])),
        ));
        AnyView::new(StorySection::new(
            "Scroll to index",
            "The viewport materializes rows around the requested index.",
            AnyView::new(cove_ui_compose::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![controls, stack])),
            )),
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
        let actions_log = self.log.clone();
        let activate_log = self.log.clone();
        let rows = (0..SWIPE_ROWS.len()).collect::<Vec<usize>>();
        let list = AnyView::new(
            SwipeList::new(
                rows,
                |index: &usize| *index,
                |index: &usize, state: RowState| {
                    let ink = if state.is_selected {
                        SemanticColorRef::accent()
                    } else {
                        SemanticColorRef::on_surface_muted()
                    };
                    AnyView::new(BoxView::new(
                        cove_core::scene::FlexDirection::Row,
                        cove_core::scene::Align::Center,
                        cove_core::scene::Justify::FlexStart,
                        cove_core::scene::Wrap::NoWrap,
                        10.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(
                                Icon::semantic_with_ref(
                                    ui_icon_catalog::shape::circle(),
                                    SemanticIconSize::small(),
                                    ink,
                                )
                                .accessibility_hidden(true),
                            ),
                            AnyView::new(BoxView::new(
                                cove_core::scene::FlexDirection::Column,
                                cove_core::scene::Align::Stretch,
                                cove_core::scene::Justify::FlexStart,
                                cove_core::scene::Wrap::NoWrap,
                                2.0,
                                AnyView::new(TupleView::new(vec![
                                    AnyView::new(
                                        Text::new(SWIPE_ROWS[*index])
                                            .semantic_foreground(SemanticColorRef::on_surface()),
                                    ),
                                    AnyView::new(
                                        Text::new("Swipe toward the leading edge or use Actions")
                                            .truncated()
                                            .semantic_font(SemanticFontRef::caption())
                                            .semantic_foreground(
                                                SemanticColorRef::on_surface_muted(),
                                            ),
                                    ),
                                ])),
                            )),
                        ])),
                    ))
                },
            )
            .selection(Selection::single(self.selection.binding()))
            .actions(move |index: &usize| {
                let row = SWIPE_ROWS[*index];
                let archive = actions_log.clone();
                let delete = actions_log.clone();
                vec![
                    SwipeAction::new(*index as u64 * 10 + 1, "Archive", move || {
                        archive.set(format!("Archived {row}"))
                    }),
                    SwipeAction::new(*index as u64 * 10 + 2, "Delete", move || {
                        delete.set(format!("Deleted {row}"))
                    })
                    .tone(StatusTone::Danger),
                ]
            })
            .on_activate(move |index: &usize| {
                activate_log.set(format!("Opened {}", SWIPE_ROWS[*index]));
            })
            .row_height(52.0)
            .row_spacing(2.0)
            .row_label(|index: &usize| SWIPE_ROWS[*index].to_owned())
            .frame_height(260.0)
            .debug_name("swipe-list"),
        );
        AnyView::new(StorySection::new(
            "Swipe actions",
            "Touch rows reveal up to two trailing actions. Every row also has an actions button and a right-click menu, so pointer, keyboard and assistive clients can run the same commands without swiping.",
            AnyView::new(cove_ui_compose::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    list,
                    AnyView::new(
                        Text::new(self.log())
                            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
                            .debug_name("swipe-log"),
                    ),
                ])),
            )),
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

    /// The refresh options the list receives. A pull calls back into the story,
    /// which owns the in-flight flag.
    pub fn options(&self) -> RefreshOptions {
        let story = self.handle();
        RefreshOptions::new(move || {
            story.start_refresh();
        })
        .threshold(REFRESH_THRESHOLD)
        .label("Refreshing…")
        .refreshing(self.is_refreshing.get())
    }

    fn handle(&self) -> Self {
        Self {
            is_refreshing: self.is_refreshing.clone(),
            refreshes: self.refreshes.clone(),
        }
    }
}

impl Default for RefreshListStory {
    fn default() -> Self {
        Self::new()
    }
}

impl View for RefreshListStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        let rows = (0..REFRESH_ROWS.len()).collect::<Vec<usize>>();
        let list = AnyView::new(
            List::new(rows, |index: &usize, _: RowState| {
                AnyView::new(Text::new(REFRESH_ROWS[*index]))
            })
            .row_height(44.0)
            .refresh(self.options())
            .frame_height(264.0)
            .debug_name("refresh-list"),
        );
        let start = self.handle();
        let refresh = AnyView::new(
            Button::title("Refresh", move || {
                start.start_refresh();
            })
            .enabled(!self.is_refreshing()),
        );
        let finish = self.handle();
        let done = AnyView::new(
            Button::title("Finish", move || finish.finish_refresh()).enabled(self.is_refreshing()),
        );
        let indicator = AnyView::new(
            Text::new(if self.is_refreshing() {
                "Refreshing…".to_owned()
            } else {
                format!("Refreshes: {}", self.refreshes())
            })
            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption())
            .debug_name("refresh-count"),
        );
        AnyView::new(StorySection::new(
            "Pull to refresh",
            "Touch pull past 64pt starts the caller-owned refresh; the indicator holds until the binding clears and freezes under reduced motion. Cmd+R or Ctrl+R is the keyboard path, and the explicit button is the pointer path.",
            AnyView::new(cove_ui_compose::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    list,
                    AnyView::new(cove_ui_compose::BoxView::new(
                        cove_core::scene::FlexDirection::Row,
                        cove_core::scene::Align::Center,
                        cove_core::scene::Justify::FlexStart,
                        cove_core::scene::Wrap::Wrap,
                        12.0,
                        AnyView::new(TupleView::new(vec![refresh, done, indicator])),
                    )),
                ])),
            )),
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
    highlight: State<Option<u64>>,
}

impl DragAndDropStory {
    pub fn new() -> Self {
        Self {
            lane_a: State::new(None),
            lane_b: State::new(None),
            highlight: State::new(None),
        }
    }

    pub fn lane_a(&self) -> Option<String> {
        self.lane_a.get()
    }

    pub fn lane_b(&self) -> Option<String> {
        self.lane_b.get()
    }

    /// The lane currently highlighted by the pointer.
    pub fn highlight(&self) -> Option<u64> {
        self.highlight.get()
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
        let lane_a = self.lane_a.clone();
        let lane_b = self.lane_b.clone();
        let chip = |title: &'static str, payload: u64| {
            let body = AnyView::new(
                AnyView::new(BoxView::new(
                    cove_core::scene::FlexDirection::Row,
                    cove_core::scene::Align::Center,
                    cove_core::scene::Justify::Center,
                    cove_core::scene::Wrap::NoWrap,
                    0.0,
                    AnyView::new(Text::new(title)),
                ))
                .frame_size(120.0, 36.0)
                .semantic_background(SemanticColorRef::surface_raised())
                .corner_radius(8.0),
            );
            AnyView::new(Draggable::new(payload, body.clone(), body))
        };
        let highlight = self.highlight.clone();
        let lane = move |title: &'static str, accepted: u64, on_drop: Rc<dyn Fn(u64)>| {
            let drop_title = title;
            let is_targeted = highlight.get() == Some(accepted);
            let fill = if is_targeted {
                SemanticColorRef::state_layer_selected()
            } else {
                SemanticColorRef::surface_variant()
            };
            let body = AnyView::new(
                AnyView::new(
                    AnyView::new(BoxView::new(
                        cove_core::scene::FlexDirection::Row,
                        cove_core::scene::Align::Center,
                        cove_core::scene::Justify::Center,
                        cove_core::scene::Wrap::NoWrap,
                        0.0,
                        AnyView::new(Text::new(drop_title)),
                    ))
                    .frame_size(160.0, 80.0)
                    .semantic_background(fill)
                    .corner_radius(10.0),
                )
                .debug_name("drop-lane"),
            );
            let targeted = highlight.clone();
            AnyView::new(
                DropTarget::new(
                    move |payload: &u64| *payload == accepted,
                    move |payload| on_drop(payload),
                    body,
                )
                .on_targeted(move |is_targeted| {
                    targeted.update(|current| {
                        if is_targeted {
                            *current = Some(accepted);
                        } else if *current == Some(accepted) {
                            *current = None;
                        }
                    });
                })
                .debug_name("drop-target"),
            )
        };
        let into_a = {
            let lane_a = lane_a.clone();
            Rc::new(move |payload: u64| {
                lane_a.set(Some(
                    if payload == 1 { "Design" } else { "Billing" }.to_owned(),
                ));
            }) as Rc<dyn Fn(u64)>
        };
        let into_b = {
            let lane_b = lane_b.clone();
            Rc::new(move |payload: u64| {
                lane_b.set(Some(
                    if payload == 1 { "Design" } else { "Billing" }.to_owned(),
                ));
            }) as Rc<dyn Fn(u64)>
        };
        let scope = AnyView::new(DragScope::new(cove_ui_compose::BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::FlexStart,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            16.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(cove_ui_compose::BoxView::new(
                    cove_core::scene::FlexDirection::Row,
                    cove_core::scene::Align::Center,
                    cove_core::scene::Justify::FlexStart,
                    cove_core::scene::Wrap::Wrap,
                    12.0,
                    AnyView::new(TupleView::new(vec![chip("Design", 1), chip("Billing", 2)])),
                )),
                AnyView::new(cove_ui_compose::BoxView::new(
                    cove_core::scene::FlexDirection::Row,
                    cove_core::scene::Align::FlexStart,
                    cove_core::scene::Justify::FlexStart,
                    cove_core::scene::Wrap::Wrap,
                    12.0,
                    AnyView::new(TupleView::new(vec![
                        lane("Design lane", 1, into_a),
                        lane("Billing lane", 2, into_b),
                    ])),
                )),
            ])),
        )));
        AnyView::new(StorySection::new(
            "Typed payloads",
            "Drag a tag into either lane. The preview is portal-rendered, the target highlights while hovered, and only a matching payload calls its drop closure. Escape is not required for a cancelled drag: releasing outside every target drops nothing.",
            AnyView::new(cove_ui_compose::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    scope,
                    AnyView::new(
                        Text::new(format!(
                            "A: {} · B: {}",
                            self.lane_a().unwrap_or_else(|| "empty".to_owned()),
                            self.lane_b().unwrap_or_else(|| "empty".to_owned())
                        ))
                        .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                    ),
                ])),
            )),
        ))
    }

    fn tag(&self) -> &'static str {
        "DragAndDropStory"
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; lane_a, lane_b, highlight)
    }
}

/// The six rows the sectioned list spreads over three sections.
pub const SECTIONED_ROWS: [&str; 6] = [
    "Design review",
    "Release notes",
    "Billing",
    "Research",
    "Archive",
    "Inbox",
];

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
        let sections = self
            .sections()
            .into_iter()
            .enumerate()
            .map(|(index, title)| {
                let rows = (index * 2..index * 2 + 2).collect::<Vec<usize>>();
                ListSection::new(index, rows).title(title)
            })
            .collect();
        let list = AnyView::new(
            SectionedList::new(
                sections,
                |index: &usize| *index,
                |index: &usize, _: RowState| AnyView::new(Text::new(SECTIONED_ROWS[*index])),
            )
            .selection(Selection::single(self.selection.binding()))
            .sticky_headers(true)
            .row_height(Some(36.0))
            .frame_height(260.0)
            .debug_name("sectioned-list"),
        );
        AnyView::new(StorySection::new(
            "Sectioned list",
            "Headers stay with their first row while scrolling.",
            AnyView::new(cove_ui_compose::BoxView::new(
                cove_core::scene::FlexDirection::Column,
                cove_core::scene::Align::Stretch,
                cove_core::scene::Justify::FlexStart,
                cove_core::scene::Wrap::NoWrap,
                12.0,
                AnyView::new(TupleView::new(vec![
                    list,
                    AnyView::new(
                        Text::new(format!("{} sections", self.sections().len()))
                            .semantic_font(cove_ui_compose::theme::SemanticFontRef::caption()),
                    ),
                ])),
            )),
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
