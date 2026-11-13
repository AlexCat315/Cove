//! Translation of CoveUIScene/Accessibility.swift.
use super::{Affine2D, Node};
use crate::Rect;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Platform-neutral meaning of a scene element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccessibilityRole {
    Group,
    Toolbar,
    Button,
    StaticText,
    Heading,
    TextField,
    Checkbox,
    Toggle,
    Slider,
    Radio,
    Tab,
    List,
    ListItem,
    Tree,
    TreeItem,
    Table,
    Row,
    Cell,
    Dialog,
    Status,
    Progress,
    Image,
    Link,
    ScrollArea,
    ComboBox,
    TabList,
    TabPanel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessibilityState {
    pub is_enabled: bool,
    pub is_selected: bool,
    pub is_expanded: Option<bool>,
    pub is_required: bool,
    pub is_invalid: bool,
    pub is_read_only: bool,
    pub is_secure: bool,
}
impl Default for AccessibilityState {
    fn default() -> Self {
        Self {
            is_enabled: true,
            is_selected: false,
            is_expanded: None,
            is_required: false,
            is_invalid: false,
            is_read_only: false,
            is_secure: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessibilitySemantics {
    pub role: AccessibilityRole,
    pub label: String,
    pub value: String,
    pub help: String,
    pub identifier: String,
    /// Identifier of another element whose label/value describes this one.
    pub described_by: Option<String>,
    pub state: AccessibilityState,
    /// A control combines decorative text, while retaining nested actionable controls.
    pub combines_children: bool,
    pub is_hidden: bool,
}
impl Default for AccessibilitySemantics {
    fn default() -> Self {
        Self {
            role: AccessibilityRole::Group,
            label: String::new(),
            value: String::new(),
            help: String::new(),
            identifier: String::new(),
            described_by: None,
            state: AccessibilityState::default(),
            combines_children: false,
            is_hidden: false,
        }
    }
}
impl AccessibilitySemantics {
    pub fn new(role: AccessibilityRole) -> Self {
        Self {
            role,
            ..Self::default()
        }
    }
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }
    pub fn with_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = identifier.into();
        self
    }
    pub fn described_by(mut self, identifier: impl Into<String>) -> Self {
        self.described_by = Some(identifier.into());
        self
    }
    pub fn combining_children(mut self) -> Self {
        self.combines_children = true;
        self
    }
    pub fn hidden(mut self) -> Self {
        self.is_hidden = true;
        self
    }
}

/// Applies a value handed in by an assistive client.
pub type AccessibilityValueSetter = std::rc::Rc<dyn Fn(&str)>;

/// Closures operate on the same controlled bindings as pointer and keyboard input.
#[derive(Clone, Default)]
pub struct AccessibilityActions {
    pub activate: Option<std::rc::Rc<dyn Fn()>>,
    pub set_value: Option<AccessibilityValueSetter>,
    pub increment: Option<std::rc::Rc<dyn Fn()>>,
    pub decrement: Option<std::rc::Rc<dyn Fn()>>,
}
impl std::fmt::Debug for AccessibilityActions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccessibilityActions")
            .field("activate", &self.activate.is_some())
            .field("set_value", &self.set_value.is_some())
            .field("increment", &self.increment.is_some())
            .field("decrement", &self.decrement.is_some())
            .finish()
    }
}

/// Large values are materialized only when an assistive client asks for them.
#[derive(Clone)]
pub struct AccessibilityValueProvider {
    pub revision: u64,
    pub read: std::rc::Rc<dyn Fn() -> String>,
}
impl AccessibilityValueProvider {
    pub fn new(revision: u64, read: impl Fn() -> String + 'static) -> Self {
        Self {
            revision,
            read: std::rc::Rc::new(read),
        }
    }
}

/// Monotonic counters for mutations that can change the accessibility tree.
pub struct AccessibilityRevision;
static STRUCTURE_REVISION: AtomicU64 = AtomicU64::new(0);
static GEOMETRY_REVISION: AtomicU64 = AtomicU64::new(0);
impl AccessibilityRevision {
    pub fn structure() -> u64 {
        STRUCTURE_REVISION.load(Ordering::Relaxed)
    }
    pub fn geometry() -> u64 {
        GEOMETRY_REVISION.load(Ordering::Relaxed)
    }
    pub fn bump_structure() {
        STRUCTURE_REVISION.fetch_add(1, Ordering::Relaxed);
        GEOMETRY_REVISION.fetch_add(1, Ordering::Relaxed);
    }
    pub fn bump_geometry() {
        GEOMETRY_REVISION.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct AccessibilityElement {
    pub node: Node,
    pub semantics: AccessibilitySemantics,
    pub frame: Rect,
    pub children: Vec<AccessibilityElement>,
    /// Whether pointer and keyboard input reach this element.
    pub accepts_input: bool,
}
impl AccessibilityElement {
    pub fn id(&self) -> super::ElementId {
        self.node.id()
    }
}

pub struct AccessibilityTree;
impl AccessibilityTree {
    pub fn snapshot(root: &Node) -> Vec<AccessibilityElement> {
        Self::snapshot_in(root, None, true)
    }
    pub fn snapshot_in(
        root: &Node,
        viewport: Option<Rect>,
        resolve_values: bool,
    ) -> Vec<AccessibilityElement> {
        let values = if resolve_values {
            Values::Resolve
        } else {
            Values::Authored
        };
        Self::collect_snapshot(root, viewport, values)
    }

    /// Application semantics for diagnostics. Field drafts and secure values
    /// are removed before label aggregation and description resolution.
    pub fn diagnostic_snapshot(root: &Node) -> Vec<AccessibilityElement> {
        Self::collect_snapshot(root, None, Values::Diagnostic)
    }

    fn collect_snapshot(
        root: &Node,
        viewport: Option<Rect>,
        values: Values,
    ) -> Vec<AccessibilityElement> {
        let mut elements = Vec::new();
        let clip = viewport.unwrap_or_else(|| root.absolute_frame());
        let parent_accepts = root
            .parent()
            .is_none_or(|parent| parent.accepts_subtree_input());
        collect(
            root,
            root.absolute_transform(),
            clip,
            false,
            parent_accepts,
            values,
            &mut elements,
        );

        let mut by_identifier: BTreeMap<String, AccessibilityElement> = BTreeMap::new();
        let mut has_descriptions = false;
        index(&elements, &mut by_identifier, &mut has_descriptions);
        if !has_descriptions {
            return elements;
        }
        resolve(elements, &by_identifier)
    }
    /// Resolves `describedBy` links for clients that read semantics directly.
    pub fn resolving_descriptions(
        mut meanings: BTreeMap<String, AccessibilitySemantics>,
    ) -> BTreeMap<String, AccessibilitySemantics> {
        let mut by_identifier: BTreeMap<String, AccessibilitySemantics> = BTreeMap::new();
        for semantics in meanings
            .values()
            .filter(|semantics| !semantics.identifier.is_empty())
        {
            by_identifier
                .entry(semantics.identifier.clone())
                .or_insert_with(|| semantics.clone());
        }
        for semantics in meanings.values_mut() {
            let Some(target) = semantics
                .described_by
                .as_ref()
                .and_then(|identifier| by_identifier.get(identifier))
            else {
                continue;
            };
            let description = if target.label.is_empty() {
                target.value.clone()
            } else {
                target.label.clone()
            };
            if !description.is_empty() && !semantics.help.contains(&description) {
                semantics.help = if semantics.help.is_empty() {
                    description
                } else {
                    format!("{}. {}", semantics.help, description)
                };
            }
        }
        meanings
    }
    /// Shared by native accessibility and agent diagnostics.
    pub fn resolved_label(node: &Node) -> String {
        let Some(semantic) = node.accessibility() else {
            return String::new();
        };
        if !(semantic.label.is_empty() && semantic.combines_children) {
            return semantic.label;
        }
        let text = label_text(node, false).join(" ");
        if text.is_empty() { semantic.help } else { text }
    }
    pub fn visible_text(node: &Node) -> String {
        label_text(node, false).join(" ")
    }
    pub fn drawn_text(node: &Node) -> String {
        label_text(node, true).join(" ")
    }
}

fn index(
    elements: &[AccessibilityElement],
    by_identifier: &mut BTreeMap<String, AccessibilityElement>,
    has_descriptions: &mut bool,
) {
    for element in elements {
        *has_descriptions |= element.semantics.described_by.is_some();
        if !element.semantics.identifier.is_empty() {
            by_identifier
                .entry(element.semantics.identifier.clone())
                .or_insert_with(|| element.clone());
        }
        index(&element.children, by_identifier, has_descriptions);
    }
}

fn resolve(
    elements: Vec<AccessibilityElement>,
    by_identifier: &BTreeMap<String, AccessibilityElement>,
) -> Vec<AccessibilityElement> {
    elements
        .into_iter()
        .map(|element| {
            let mut semantics = element.semantics;
            if let Some(target) = semantics
                .described_by
                .as_ref()
                .and_then(|identifier| by_identifier.get(identifier))
            {
                let description = if target.semantics.label.is_empty() {
                    target.semantics.value.clone()
                } else {
                    target.semantics.label.clone()
                };
                if !description.is_empty() && !semantics.help.contains(&description) {
                    semantics.help = if semantics.help.is_empty() {
                        description
                    } else {
                        format!("{}. {}", semantics.help, description)
                    };
                }
            }
            AccessibilityElement {
                node: element.node,
                semantics,
                frame: element.frame,
                children: resolve(element.children, by_identifier),
                accepts_input: element.accepts_input,
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Values {
    Resolve,
    Authored,
    Diagnostic,
}

fn collect(
    node: &Node,
    transform: Affine2D,
    clip: Rect,
    combines_text: bool,
    parent_accepts_input: bool,
    values: Values,
    output: &mut Vec<AccessibilityElement>,
) {
    let semantics = node.accessibility();
    if !node.is_layout_visible()
        || !node.is_interaction_enabled()
        || node.opacity() <= 0.0
        || node.subtree_opacity() <= 0.0
        || semantics
            .as_ref()
            .is_some_and(|semantics| semantics.is_hidden)
    {
        return;
    }
    let accepts_input = parent_accepts_input
        && node.is_layout_visible()
        && node.allows_hit_testing()
        && node.is_interaction_enabled();
    let frame = node.frame();
    let bounds = Rect::new(
        transform.e,
        transform.f,
        frame.width * transform.a.abs(),
        frame.height * transform.d.abs(),
    );
    let child_clip = if node.clips_to_bounds() {
        clip.intersection(bounds)
    } else {
        Some(clip)
    };
    let Some(child_clip) = child_clip else { return };

    let mut content = transform;
    let offset = node.content_offset();
    content.e -= offset.x * transform.a;
    content.f -= offset.y * transform.d;

    let child_combines = combines_text
        || semantics
            .as_ref()
            .is_some_and(|semantics| semantics.combines_children);
    let mut children = Vec::new();
    for child in node.children() {
        let child_transform =
            super::placement_step(child.frame(), child.transform(), crate::Point::default())
                .followed(content);
        collect(
            &child,
            child_transform,
            child_clip,
            child_combines,
            accepts_input,
            values,
            &mut children,
        );
    }

    let Some(frame) = bounds.intersection(clip) else {
        // A node clipped out of view still contributes its descendants, which
        // may lie inside the clip. Swift does the same in `appendChildren`.
        output.extend(children);
        return;
    };
    let Some(mut semantic) = semantics else {
        output.extend(children);
        return;
    };
    if combines_text && semantic.role == AccessibilityRole::StaticText {
        output.extend(children);
        return;
    }
    if frame.width <= 0.0 || frame.height <= 0.0 {
        output.extend(children);
        return;
    }
    if semantic.label.is_empty() && semantic.combines_children {
        semantic.label = AccessibilityTree::resolved_label(node);
    }
    if semantic.role == AccessibilityRole::Cell && semantic.value.is_empty() {
        semantic.value = label_text(node, false).join(" ");
    }
    if semantic.role == AccessibilityRole::ListItem && semantic.label.is_empty() {
        semantic.label = children
            .iter()
            .filter(|child| child.semantics.role == AccessibilityRole::Cell)
            .map(|child| format!("{}: {}", child.semantics.label, child.semantics.value))
            .collect::<Vec<_>>()
            .join(", ");
    }
    if values == Values::Diagnostic
        && (semantic.state.is_secure || semantic.role == AccessibilityRole::TextField)
    {
        semantic.value.clear();
    } else if values == Values::Resolve && node.accessibility_value_provider().is_some() {
        semantic.value = node.resolved_accessibility_value();
    } else if semantic.state.is_secure {
        semantic.value = "•".repeat(semantic.value.chars().count());
    }
    output.push(AccessibilityElement {
        node: node.clone(),
        semantics: semantic,
        frame,
        children,
        accepts_input,
    });
}

fn label_text(node: &Node, includes_hidden_text: bool) -> Vec<String> {
    let mut out = Vec::new();
    for child in node.children() {
        if !child.is_layout_visible()
            || !child.is_interaction_enabled()
            || child.opacity() <= 0.0
            || child.subtree_opacity() <= 0.0
        {
            continue;
        }
        if let Some(semantics) = child.accessibility() {
            if semantics.is_hidden && !includes_hidden_text {
                continue;
            }
            if semantics.role == AccessibilityRole::StaticText {
                out.push(semantics.label);
                continue;
            }
            if semantics.role != AccessibilityRole::Group
                && !(semantics.role == AccessibilityRole::ScrollArea && semantics.combines_children)
            {
                continue;
            }
        }
        out.extend(label_text(&child, includes_hidden_text));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revisions_are_monotonic() {
        let structure = AccessibilityRevision::structure();
        AccessibilityRevision::bump_geometry();
        assert_eq!(AccessibilityRevision::structure(), structure);
        AccessibilityRevision::bump_structure();
        assert!(AccessibilityRevision::structure() > structure);
    }
    #[test]
    fn descriptions_fall_back_from_label_to_value() {
        let mut meanings = BTreeMap::new();
        meanings.insert(
            "a".to_owned(),
            AccessibilitySemantics::new(AccessibilityRole::StaticText).with_identifier("label.a"),
        );
        meanings.insert(
            "b".to_owned(),
            AccessibilitySemantics::new(AccessibilityRole::Button).described_by("label.a"),
        );
        let resolved = AccessibilityTree::resolving_descriptions(meanings);
        assert_eq!(resolved["b"].help, "");
        let mut meanings = BTreeMap::new();
        meanings.insert(
            "a".to_owned(),
            AccessibilitySemantics::new(AccessibilityRole::StaticText)
                .with_value("value")
                .with_identifier("label.a"),
        );
        meanings.insert(
            "b".to_owned(),
            AccessibilitySemantics::new(AccessibilityRole::Button).described_by("label.a"),
        );
        let resolved = AccessibilityTree::resolving_descriptions(meanings);
        assert_eq!(resolved["b"].help, "value");
    }
}
