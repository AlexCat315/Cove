//! Structural rope comparison. Boundary paths are walked so identical
//! persistent subtrees are skipped in O(1); byte comparisons are confined to
//! changed leaves and non-shared input text.

use super::rope_storage::{RopeHandle, RopeStorage};
use std::rc::Rc;

struct Cursor {
    nodes: Vec<RopeHandle>,
    byte_offset: usize,
    from_end: bool,
}

impl Cursor {
    fn new(root: RopeHandle, from_end: bool) -> Self {
        Self {
            nodes: vec![root],
            byte_offset: 0,
            from_end,
        }
    }

    fn expand(&mut self) {
        let Some(node) = self.nodes.last().cloned() else {
            return;
        };
        let RopeStorage::Branch(branch) = &node.storage else {
            return;
        };
        self.nodes.pop();
        if self.from_end {
            self.nodes.push(branch.left.clone());
            self.nodes.push(branch.right.clone());
        } else {
            self.nodes.push(branch.right.clone());
            self.nodes.push(branch.left.clone());
        }
    }

    fn consume(&mut self, count: usize) {
        self.byte_offset += count;
        if let Some(node) = self.nodes.last() {
            if self.byte_offset == node.metrics.utf8_length {
                self.nodes.pop();
                self.byte_offset = 0;
            }
        }
    }
}

/// Number of identical bytes from the start or end of both roots, bounded by
/// `limit`.
pub fn common_bytes(
    previous: &RopeHandle,
    current: &RopeHandle,
    from_end: bool,
    limit: usize,
) -> usize {
    let mut a = Cursor::new(previous.clone(), from_end);
    let mut b = Cursor::new(current.clone(), from_end);
    let mut matched = 0usize;
    while matched < limit {
        let (Some(left), Some(right)) = (a.nodes.last().cloned(), b.nodes.last().cloned()) else {
            break;
        };
        if Rc::ptr_eq(&left, &right) && a.byte_offset == 0 && b.byte_offset == 0 {
            let count = left.metrics.utf8_length.min(limit - matched);
            a.consume(count);
            b.consume(count);
            matched += count;
            continue;
        }
        if left.metrics.height > 1 || right.metrics.height > 1 {
            if left.metrics.height >= right.metrics.height {
                a.expand();
            }
            if right.metrics.height >= left.metrics.height {
                b.expand();
            }
            continue;
        }
        let (RopeStorage::Leaf(la), RopeStorage::Leaf(lb)) = (&left.storage, &right.storage)
        else {
            break;
        };
        let count = (la.bytes.len() - a.byte_offset)
            .min(lb.bytes.len() - b.byte_offset)
            .min(limit - matched);
        if count == 0 {
            return matched;
        }
        for index in 0..count {
            let ai = if from_end {
                la.bytes.len() - 1 - a.byte_offset - index
            } else {
                a.byte_offset + index
            };
            let bi = if from_end {
                lb.bytes.len() - 1 - b.byte_offset - index
            } else {
                b.byte_offset + index
            };
            if la.bytes[ai] != lb.bytes[bi] {
                return matched + index;
            }
        }
        a.consume(count);
        b.consume(count);
        matched += count;
    }
    matched
}
