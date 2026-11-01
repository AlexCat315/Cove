use cove_core::scene::NodeTree;
use cove_shell::ShellError;
use std::{cell::RefCell, rc::Rc};
pub trait PlatformHost {
    fn run(&self, tree: Rc<RefCell<NodeTree>>) -> Result<(), ShellError>;
    fn stop(&self);
    fn is_running(&self) -> bool;
}
