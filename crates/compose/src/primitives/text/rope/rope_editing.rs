//! Persistent AVL join/split with grapheme-stable seams.

use super::rope_storage::{RopeHandle, RopeLeaf, RopeNode, RopeStorage, empty_node};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// Height-aware AVL join visits only the taller tree's boundary path.
pub fn join(left: RopeHandle, right: RopeHandle) -> RopeHandle {
    if left.metrics.utf8_length == 0 {
        return right;
    }
    if right.metrics.utf8_length == 0 {
        return left;
    }
    if left.metrics.height > right.metrics.height + 1
        && let RopeStorage::Branch(branch) = &left.storage
    {
        return Arc::new(balance(
            branch.left.clone(),
            join(branch.right.clone(), right),
        ));
    }
    if right.metrics.height > left.metrics.height + 1
        && let RopeStorage::Branch(branch) = &right.storage
    {
        return Arc::new(balance(
            join(left, branch.left.clone()),
            branch.right.clone(),
        ));
    }
    if let (RopeStorage::Leaf(a), RopeStorage::Leaf(b)) = (&left.storage, &right.storage)
        && a.bytes.len() + b.bytes.len() <= RopeNode::MAX_LEAF_BYTES
    {
        let mut text = a.text.clone();
        text.push_str(&b.text);
        return Arc::new(RopeNode::with_leaf(Arc::new(RopeLeaf::new(text))));
    }
    Arc::new(RopeNode::new(left, right))
}

fn balance(left: RopeHandle, right: RopeHandle) -> RopeNode {
    if left.metrics.height > right.metrics.height + 1
        && let RopeStorage::Branch(a) = &left.storage
    {
        if a.left.metrics.height >= a.right.metrics.height {
            return RopeNode::new(
                a.left.clone(),
                Arc::new(RopeNode::new(a.right.clone(), right)),
            );
        }
        if let RopeStorage::Branch(b) = &a.right.storage {
            return RopeNode::new(
                Arc::new(RopeNode::new(a.left.clone(), b.left.clone())),
                Arc::new(RopeNode::new(b.right.clone(), right)),
            );
        }
    }
    if right.metrics.height > left.metrics.height + 1
        && let RopeStorage::Branch(a) = &right.storage
    {
        if a.right.metrics.height >= a.left.metrics.height {
            return RopeNode::new(
                Arc::new(RopeNode::new(left, a.left.clone())),
                a.right.clone(),
            );
        }
        if let RopeStorage::Branch(b) = &a.left.storage {
            return RopeNode::new(
                Arc::new(RopeNode::new(left, b.left.clone())),
                Arc::new(RopeNode::new(b.right.clone(), a.right.clone())),
            );
        }
    }
    RopeNode::new(left, right)
}

/// An insertion/deletion can merge graphemes across the edited seam. Repair its
/// edge leaves, including a regional-indicator run whose pairing changes.
pub fn concatenate(left: RopeHandle, right: RopeHandle) -> RopeHandle {
    if left.metrics.utf8_length == 0 {
        return right;
    }
    if right.metrics.utf8_length == 0 {
        return left;
    }
    let left_edge = left.last_leaf();
    let right_edge = right.first_leaf();
    if stable_boundary(&left_edge.text, &right_edge.text) {
        return join(left, right);
    }
    let (prefix, _) = split(
        left.clone(),
        left.metrics
            .character_count
            .saturating_sub(left_edge.character_count()),
    );
    let mut text = left_edge.text.clone();
    let mut tail = right;
    loop {
        let leaf = tail.first_leaf();
        text.push_str(&leaf.text);
        tail = split(tail.clone(), leaf.character_count()).1;
        if tail.metrics.utf8_length == 0 {
            break;
        }
        if stable_boundary(&text, &tail.first_leaf().text) {
            break;
        }
    }
    join(join(prefix, RopeNode::build(&text)), tail)
}

fn stable_boundary(left: &str, right: &str) -> bool {
    let (Some(last), Some(first)) = (
        left.graphemes(true).next_back(),
        right.graphemes(true).next(),
    ) else {
        return true;
    };
    // Counting alone misses RI re-pairing (three indicators still form two
    // characters). The first cluster's exact bytes must remain unchanged.
    let mut boundary = String::with_capacity(last.len() + first.len());
    boundary.push_str(last);
    boundary.push_str(first);
    match boundary.graphemes(true).next() {
        Some(cluster) => cluster.as_bytes() == last.as_bytes(),
        None => true,
    }
}

pub fn split(node: RopeHandle, index: usize) -> (RopeHandle, RopeHandle) {
    if index == 0 {
        return (empty_node(), node);
    }
    if index >= node.metrics.character_count {
        return (node, empty_node());
    }
    match &node.storage {
        RopeStorage::Leaf(leaf) => {
            let byte = leaf.character_boundaries[index];
            let left_text = std::str::from_utf8(&leaf.bytes[..byte])
                .expect("rope leaf prefix is valid UTF-8")
                .to_owned();
            let right_text = std::str::from_utf8(&leaf.bytes[byte..])
                .expect("rope leaf suffix is valid UTF-8")
                .to_owned();
            (
                Arc::new(RopeNode::with_leaf(Arc::new(RopeLeaf::new(left_text)))),
                Arc::new(RopeNode::with_leaf(Arc::new(RopeLeaf::new(right_text)))),
            )
        }
        RopeStorage::Branch(branch) => {
            if index < branch.left.metrics.character_count {
                let (a, b) = split(branch.left.clone(), index);
                (a, join(b, branch.right.clone()))
            } else {
                let (a, b) = split(
                    branch.right.clone(),
                    index - branch.left.metrics.character_count,
                );
                (join(branch.left.clone(), a), b)
            }
        }
    }
}

/// Replaces a character range, preserving grapheme stability at both seams.
pub fn replace_range(
    root: RopeHandle,
    range: std::ops::Range<usize>,
    replacement: &str,
) -> RopeHandle {
    let start = range.start.min(root.metrics.character_count);
    let end = range.end.min(root.metrics.character_count);
    let (prefix, rest) = split(root, start);
    let (_, suffix) = split(rest, end.saturating_sub(start));
    concatenate(concatenate(prefix, RopeNode::build(replacement)), suffix)
}
