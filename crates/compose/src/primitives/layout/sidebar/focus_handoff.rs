//! Translation of Primitives/Layout/Sidebar/FocusHandoff.swift.
//!
//! Tracks focus at the shell boundary before retained slots change visibility.
//! Node references never escape the component or enter the caller's session.
use std::cell::{Cell, RefCell};

use cove_core::scene::{FocusChain, Node};

use super::adaptive_options::SidebarMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandoffDestination {
    Trigger,
    Navigation,
}

/// Shell-boundary focus memory for a sidebar whose slots come and go.
#[derive(Default)]
pub struct SidebarFocusHandoff {
    navigation: RefCell<Option<Node>>,
    trigger: RefCell<Option<Node>>,
    resizer: RefCell<Option<Node>>,
    previous: RefCell<Option<Node>>,
    chain: RefCell<Option<std::rc::Rc<FocusChain>>>,
    mode: Cell<Option<SidebarMode>>,
    destination: Cell<Option<HandoffDestination>>,
    visible: Cell<bool>,
    is_modal: Cell<bool>,
}

impl SidebarFocusHandoff {
    pub fn new() -> Self {
        Self {
            visible: Cell::new(true),
            ..Default::default()
        }
    }

    pub fn set_navigation(&self, node: &Node) {
        *self.navigation.borrow_mut() = Some(node.clone());
    }

    pub fn set_trigger(&self, node: &Node) {
        *self.trigger.borrow_mut() = Some(node.clone());
    }

    pub fn set_resizer(&self, node: &Node) {
        *self.resizer.borrow_mut() = Some(node.clone());
    }

    /// Record the focus owner before the slots change visibility, so
    /// `complete` can hand off only if it actually became inactive.
    pub fn prepare(
        &self,
        next: SidebarMode,
        is_modal: bool,
        chain: Option<std::rc::Rc<FocusChain>>,
    ) {
        let previous_mode = self.mode.get();
        self.mode.set(Some(next));
        self.is_modal.set(is_modal);
        let Some(chain) = chain else { return };
        *self.chain.borrow_mut() = Some(chain.clone());
        let previous_mode = match previous_mode {
            Some(mode) => mode,
            None => return,
        };
        if previous_mode == next {
            return;
        }
        let Some(focused) = chain.focused() else {
            return;
        };
        let destination = if self
            .resizer
            .borrow()
            .as_ref()
            .is_some_and(|resizer| *resizer == focused)
        {
            if next == SidebarMode::Expanded {
                return;
            }
            Some(if next == SidebarMode::Compact && !is_modal {
                HandoffDestination::Trigger
            } else {
                HandoffDestination::Navigation
            })
        } else if Self::contains(&focused, self.navigation.borrow().as_ref()) {
            Some(if next == SidebarMode::Compact && !is_modal {
                HandoffDestination::Trigger
            } else {
                HandoffDestination::Navigation
            })
        } else if Self::contains(&focused, self.trigger.borrow().as_ref())
            && next != SidebarMode::Compact
        {
            Some(HandoffDestination::Navigation)
        } else {
            None
        };
        let Some(destination) = destination else {
            return;
        };
        *self.previous.borrow_mut() = Some(focused);
        self.visible.set(chain.is_focus_visible());
        self.destination.set(Some(destination));
    }

    /// Hand off focus once the new presentation has settled.
    pub fn complete(&self, chain: Option<std::rc::Rc<FocusChain>>) {
        let Some(chain) = chain else { return };
        // A slot's own container query can settle one composition after the
        // shell. Hand off only if the remembered focus actually became inactive.
        if self.destination.get().is_none() && chain.focused().is_none() {
            if let Some(previous) = self.previous.borrow().clone()
                && !previous.accepts_subtree_input()
            {
                let mode = self.mode.get();
                if Self::contains(&previous, self.navigation.borrow().as_ref()) {
                    self.destination.set(Some(
                        if mode == Some(SidebarMode::Compact) && !self.is_modal.get() {
                            HandoffDestination::Trigger
                        } else {
                            HandoffDestination::Navigation
                        },
                    ));
                } else if Self::contains(&previous, self.trigger.borrow().as_ref())
                    && mode != Some(SidebarMode::Compact)
                {
                    self.destination.set(Some(HandoffDestination::Navigation));
                }
            }
        }
        let Some(destination) = self.destination.get() else {
            return;
        };
        let target = match destination {
            HandoffDestination::Trigger => self.trigger.borrow().clone(),
            HandoffDestination::Navigation => self.first_focusable_in_navigation(),
        };
        self.destination.set(None);
        *self.previous.borrow_mut() = None;
        if let Some(target) = target {
            chain.focus(Some(&target), self.visible.get());
        }
    }

    /// Move focus to the control whose accessible label matches `label`, used
    /// when a row disappears and the shell needs to recover.
    pub fn focus_opener(&self, label: &str) {
        let Some(chain) = self.chain.borrow().clone() else {
            return;
        };
        let visible = chain.is_focus_visible();
        for root in [
            self.trigger.borrow().clone(),
            self.navigation.borrow().clone(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(opener) = Self::focusables(&root).into_iter().find(|candidate| {
                candidate
                    .accessibility()
                    .is_some_and(|semantics| semantics.label == label)
            }) {
                chain.focus(Some(&opener), visible);
                return;
            }
        }
    }

    fn first_focusable_in_navigation(&self) -> Option<Node> {
        Self::focusables(self.navigation.borrow().as_ref()?)
            .into_iter()
            .next()
    }

    fn focusables(root: &Node) -> Vec<Node> {
        let mut result = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(candidate) = pending.pop() {
            if candidate.is_focusable() {
                result.push(candidate.clone());
            }
            pending.extend(candidate.children());
        }
        result
    }

    fn contains(candidate: &Node, ancestor: Option<&Node>) -> bool {
        let Some(ancestor) = ancestor else {
            return false;
        };
        let mut current = Some(candidate.clone());
        while let Some(node) = current {
            if node == *ancestor {
                return true;
            }
            current = node.parent();
        }
        false
    }
}
