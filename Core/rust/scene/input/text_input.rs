//! Translation of CoveUIScene/TextInputAttachment.swift and TextInputClient.swift.
use crate::scene::Node;
use crate::Point;
use cove_platform::TextInputArea;
use serde::{Deserialize, Serialize};
use std::rc::Rc;

pub type TextInputAreaResolver = Rc<dyn Fn(&Node, Point) -> Option<TextInputArea>>;
pub type TextInputFocusChangeHandler = Rc<dyn Fn(bool)>;
pub type TextInputEditingChangeHandler = Rc<dyn Fn(bool)>;

/// Undo/redo commands shared by the menu bar and text shortcuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditCommand {
    Undo,
    Redo,
}

/// Connects platform menu commands to the same history used by text shortcuts.
#[derive(Clone)]
pub struct TextEditActions {
    pub can_perform: Rc<dyn Fn(TextEditCommand) -> bool>,
    pub perform: Rc<dyn Fn(TextEditCommand)>,
}
impl TextEditActions {
    pub fn new(can_perform: impl Fn(TextEditCommand) -> bool + 'static, perform: impl Fn(TextEditCommand) + 'static) -> Self {
        Self { can_perform: Rc::new(can_perform), perform: Rc::new(perform) }
    }
}

/// Shared attachment keys for focused text-input primitives.
pub struct TextInputAttachmentKey;
impl TextInputAttachmentKey {
    /// `Node.attachments` entry carrying the current focused text input area.
    pub const AREA: &'static str = "__text_input_area";
    /// Commit-phase resolver that maps a laid-out node into the IME anchor rect.
    pub const AREA_RESOLVER: &'static str = "__text_input_area_resolver";
    /// Focus change sink owned by the host view state rather than the draw path.
    pub const FOCUS_CHANGE_HANDLER: &'static str = "__text_input_focus_change_handler";
    /// Current editing/composition state sink owned by the host view state.
    pub const EDITING_CHANGE_HANDLER: &'static str = "__text_input_editing_change_handler";
    pub const EDIT_ACTIONS: &'static str = "__text_input_edit_actions";
    pub const CLIENT: &'static str = "__text_input_client";
}

/// A focused editor's UTF-16 view for platform input methods.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextInputSnapshot {
    pub text: String,
    pub selection_start: i32,
    pub selection_end: i32,
    pub composition_start: i32,
    pub composition_end: i32,
    pub is_secure: bool,
    pub is_read_only: bool,
    pub is_multiline: bool,
}
impl TextInputSnapshot {
    pub fn new(text: impl Into<String>, selection_start: i32, selection_end: i32) -> Self {
        Self { text: text.into(), selection_start, selection_end, composition_start: -1, composition_end: -1, ..Self::default() }
    }
}

/// The editor owns text, grapheme boundaries, history and composition.
#[derive(Clone)]
pub struct TextInputClient {
    pub snapshot: Rc<dyn Fn() -> TextInputSnapshot>,
    pub select: Rc<dyn Fn(i32, i32)>,
    pub delete_surrounding: Rc<dyn Fn(i32, i32)>,
}
impl TextInputClient {
    pub fn new(snapshot: impl Fn() -> TextInputSnapshot + 'static, select: impl Fn(i32, i32) + 'static, delete_surrounding: impl Fn(i32, i32) + 'static) -> Self {
        Self { snapshot: Rc::new(snapshot), select: Rc::new(select), delete_surrounding: Rc::new(delete_surrounding) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn client_round_trips_through_attachments() {
        let node = Node::new();
        let text = Rc::new(RefCell::new(String::from("hi")));
        let stored = text.clone();
        node.set_attachment(
            TextInputAttachmentKey::CLIENT,
            TextInputClient::new(
                move || TextInputSnapshot::new(stored.borrow().clone(), 2, 2),
                |_, _| {},
                |_, _| {},
            ),
        );
        let client: TextInputClient = node.attachment(TextInputAttachmentKey::CLIENT).unwrap();
        assert_eq!((client.snapshot)().text, "hi");
        *text.borrow_mut() = "changed".into();
        assert_eq!((client.snapshot)().selection_start, 2);
    }
}
