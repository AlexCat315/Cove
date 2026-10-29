use std::{cell::RefCell,rc::Rc};
use cove_core::scene::NodeTree;
use cove_shell::ShellError;
pub trait PlatformHost{fn run(&self,tree:Rc<RefCell<NodeTree>>)->Result<(),ShellError>;fn stop(&self);fn is_running(&self)->bool;}
