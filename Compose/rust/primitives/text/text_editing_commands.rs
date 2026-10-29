//! Platform menu/shortcut bridge for text undo and redo.

use cove_core::scene::{FocusChainHolder, Node, TextEditCommand};
use std::rc::Rc;

pub struct TextEditingCommands;

impl TextEditingCommands {
    pub const UNDO_KEY: &'static str = "text.undo";
    pub const REDO_KEY: &'static str = "text.redo";
    pub const CAN_UNDO_KEY: &'static str = "text.canUndo";
    pub const CAN_REDO_KEY: &'static str = "text.canRedo";

    pub fn can_undo() -> Option<bool> {
        let chain = FocusChainHolder::current();
        Self::availability(chain.as_ref(), TextEditCommand::Undo, Self::CAN_UNDO_KEY)
    }

    pub fn can_redo() -> Option<bool> {
        let chain = FocusChainHolder::current();
        Self::availability(chain.as_ref(), TextEditCommand::Redo, Self::CAN_REDO_KEY)
    }

    fn availability(
        chain: Option<&Rc<cove_core::scene::FocusChain>>,
        command: TextEditCommand,
        attachment_key: &str,
    ) -> Option<bool> {
        if let Some(available) = chain.and_then(|chain| chain.text_edit_availability(command)) {
            return Some(available);
        }
        let target = chain.and_then(|chain| chain.command_target())?;
        let handler: Rc<dyn Fn() -> bool> = target.attachment(attachment_key)?;
        Some(handler())
    }

    pub fn undo() -> bool {
        if FocusChainHolder::current()
            .is_some_and(|chain| chain.perform_text_edit(TextEditCommand::Undo))
        {
            return true;
        }
        Self::perform(Self::UNDO_KEY)
    }

    pub fn redo() -> bool {
        if FocusChainHolder::current()
            .is_some_and(|chain| chain.perform_text_edit(TextEditCommand::Redo))
        {
            return true;
        }
        Self::perform(Self::REDO_KEY)
    }

    fn perform(key: &str) -> bool {
        let Some(action) = Self::command_target_attachment::<Rc<dyn Fn()>>(key) else {
            return false;
        };
        action();
        true
    }

    fn command_target_attachment<T: Clone + 'static>(key: &str) -> Option<T> {
        let target: Node = FocusChainHolder::current()?.command_target()?;
        target.attachment(key)
    }
}
