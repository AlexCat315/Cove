//! Translation of the configuration groups in Primitives/Data/Lists/TreeOptions.swift.
use std::collections::HashSet;
use std::rc::Rc;

use cove_core::Binding;
use cove_composition::AnyView;
use cove_platform::KeyEvent;

use super::{TreeNodeKey, TreeDropPosition};
use crate::theme::TextScaler;

/// Caller-owned selection and expansion, independent of transient pointer/drag state.
pub struct TreeSelection<ID: std::hash::Hash + Eq + Clone + Send + Sync + 'static> {
    pub primary: Binding<Option<ID>>,
    pub multiple: Option<Binding<HashSet<ID>>>,
    pub expanded: Option<Binding<HashSet<ID>>>,
    pub primary_key: Binding<Option<TreeNodeKey<ID>>>,
    pub multiple_keys: Option<Binding<HashSet<TreeNodeKey<ID>>>>,
    pub expanded_keys: Option<Binding<HashSet<TreeNodeKey<ID>>>>,
}

impl<ID: std::hash::Hash + Eq + Clone + Send + Sync + 'static> Default for TreeSelection<ID> {
    fn default() -> Self {
        Self {
            primary: Binding::constant(None),
            multiple: None,
            expanded: None,
            primary_key: Binding::constant(None),
            multiple_keys: None,
            expanded_keys: None,
        }
    }
}

/// Geometry consumed by virtualization, hit testing, guides and disclosure slots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeLayout {
    pub row_height: f32,
    pub row_spacing: f32,
    pub indentation: f32,
    pub disclosure_width: f32,
    pub shows_indent_guides: bool,
    pub trailing_slot_width: f32,
}

impl Default for TreeLayout {
    fn default() -> Self {
        Self {
            row_height: 30.0,
            row_spacing: 0.0,
            indentation: 16.0,
            disclosure_width: 18.0,
            shows_indent_guides: true,
            trailing_slot_width: 64.0,
        }
    }
}

/// Inputs to `TreeLayout::resolved` that the platform host supplies.
#[derive(Clone, Copy)]
pub struct TreeLayoutEnvironment {
    /// Small icon edge length.
    pub icon_small: f32,
    /// Icon scaling factor.
    pub icon_factor: f32,
    /// Whether the surface is touch-driven.
    pub is_touch: bool,
    /// Minimum control extent on a touch surface.
    pub minimum_control_extent: f32,
}

impl TreeLayout {
    /// Resolve one geometry so scaling cannot drift between the subsystems.
    pub fn resolved(&self, text_scaler: &TextScaler, environment: TreeLayoutEnvironment) -> Self {
        let mut result = *self;
        result.row_height = text_scaler
            .scale(self.row_height)
            .max(environment.icon_small + 4.0);
        result.row_spacing = text_scaler.scale(self.row_spacing);
        result.indentation = self.indentation * text_scaler.factor().max(environment.icon_factor);
        result.disclosure_width = if self.disclosure_width > 0.0 {
            self.disclosure_width.max(environment.icon_small)
        } else {
            0.0
        };
        result.trailing_slot_width = text_scaler.scale(self.trailing_slot_width);
        if environment.is_touch {
            result.row_height = result.row_height.max(environment.minimum_control_extent);
            if result.disclosure_width > 0.0 {
                result.disclosure_width =
                    result.disclosure_width.max(environment.minimum_control_extent);
            }
        }
        result
    }

    pub fn validate(&mut self) {
        self.row_height = self.row_height.max(1.0);
        self.row_spacing = self.row_spacing.max(0.0);
        self.indentation = self.indentation.max(0.0);
        self.disclosure_width = self.disclosure_width.max(0.0);
        self.trailing_slot_width = self.trailing_slot_width.max(0.0);
    }
}

/// Caller-supplied row chrome.
pub struct TreeSlots<Element: 'static> {
    /// Receives whether the row is expanded.
    pub disclosure: Option<Rc<dyn Fn(bool) -> AnyView>>,
    /// Receives the element, selected/focused/expanded/disabled flags and depth.
    pub trailing: Option<Rc<dyn Fn(Element, bool, bool, bool, bool, usize) -> AnyView>>,
}

impl<Element: 'static> Default for TreeSlots<Element> {
    fn default() -> Self {
        Self { disclosure: None, trailing: None }
    }
}

/// Search configuration.
pub struct TreeSearch<Element: 'static> {
    pub query: String,
    pub text: Option<Rc<dyn Fn(&Element) -> String>>,
    pub policy: super::TreeSearchFilterPolicy,
}

impl<Element: 'static> Default for TreeSearch<Element> {
    fn default() -> Self {
        Self {
            query: String::new(),
            text: None,
            policy: super::TreeSearchFilterPolicy::FilterAndAutoExpand,
        }
    }
}

/// Selection and key-command events.
pub struct TreeEvents<Element: 'static, ID: std::hash::Hash + Eq + 'static> {
    pub on_key_command: Option<Rc<dyn Fn(&KeyEvent, &HashSet<ID>) -> bool>>,
    pub on_select: Option<Rc<dyn Fn(Element)>>,
}

impl<Element: 'static, ID: std::hash::Hash + Eq + 'static> Default for TreeEvents<Element, ID> {
    fn default() -> Self {
        Self { on_key_command: None, on_select: None }
    }
}

/// Drag-and-drop operations between rows.
pub struct TreeDragOperations<Element: Clone + 'static> {
    pub can_drop: Option<Rc<dyn Fn(&Element, &Element, TreeDropPosition) -> bool>>,
    pub on_drop: Option<Rc<dyn Fn(Element, Element, TreeDropPosition)>>,
}

impl<Element: Clone + 'static> Default for TreeDragOperations<Element> {
    fn default() -> Self {
        Self { can_drop: None, on_drop: None }
    }
}

/// Configuration consists of independently reusable groups.
pub struct TreeOptions<Element: Clone + 'static, ID: std::hash::Hash + Eq + Clone + Send + Sync + 'static>
{
    pub is_enabled: bool,
    pub selection: TreeSelection<ID>,
    pub layout: TreeLayout,
    pub slots: TreeSlots<Element>,
    pub search: TreeSearch<Element>,
    pub events: TreeEvents<Element, ID>,
    pub drag: TreeDragOperations<Element>,
}

impl<Element: Clone + 'static, ID: std::hash::Hash + Eq + Clone + Send + Sync + 'static> Default
    for TreeOptions<Element, ID>
{
    fn default() -> Self {
        Self {
            is_enabled: true,
            selection: TreeSelection::default(),
            layout: TreeLayout::default(),
            slots: TreeSlots::default(),
            search: TreeSearch::default(),
            events: TreeEvents::default(),
            drag: TreeDragOperations::default(),
        }
    }
}
