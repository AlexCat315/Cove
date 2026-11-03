//! Valid UTF-8 rope storage, cut only at extended grapheme boundaries. A single
//! grapheme can exceed the nominal chunk size (for example a long sequence of
//! combining marks); preserving that grapheme takes precedence over the
//! 1024-byte packing target.

pub use cove_core::line_breaks::is_newline;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// A single immutable leaf of the rope.
pub struct RopeLeaf {
    pub bytes: Vec<u8>,
    pub text: String,
    pub character_boundaries: Vec<usize>,
    pub newline_characters: Vec<usize>,
    pub utf16_length: usize,
    pub line_feed_bytes: Vec<usize>,
}

impl RopeLeaf {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let bytes = text.as_bytes().to_vec();
        let utf16_length = text.encode_utf16().count();
        // Swift's `text.enumerated()` yields `Character` values (extended
        // grapheme clusters); CRLF is one character and one newline.
        let mut boundaries = Vec::with_capacity(text.graphemes(true).count() + 1);
        boundaries.push(0);
        let mut newlines = Vec::new();
        for (index, (byte, grapheme)) in text.grapheme_indices(true).enumerate() {
            if is_newline(grapheme) {
                newlines.push(index);
            }
            boundaries.push(byte + grapheme.len());
        }
        let line_feed_bytes = bytes
            .iter()
            .enumerate()
            .filter_map(|(offset, value)| (*value == 10).then_some(offset))
            .collect();
        Self {
            text,
            bytes,
            character_boundaries: boundaries,
            newline_characters: newlines,
            utf16_length,
            line_feed_bytes,
        }
    }

    pub fn character_count(&self) -> usize {
        self.character_boundaries.len().saturating_sub(1)
    }
}

/// A parser can read from the requested byte without materializing the document.
pub struct RopeChunk {
    pub leaf: Arc<RopeLeaf>,
    pub utf8_offset: usize,
    pub start_index: usize,
}

impl RopeChunk {
    pub fn bytes(&self) -> &[u8] {
        &self.leaf.bytes[self.start_index..]
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RopeMetrics {
    pub utf8_length: usize,
    pub character_count: usize,
    pub newline_count: usize,
    pub line_feed_count: usize,
    pub utf16_length: usize,
    pub height: usize,
}

pub struct RopeBranch {
    pub left: RopeHandle,
    pub right: RopeHandle,
}

pub enum RopeStorage {
    Leaf(Arc<RopeLeaf>),
    Branch(RopeBranch),
}

/// An immutable node in the persistent balanced rope. Cheap to clone; interior
/// mutability is not required because every edit produces a new root.
pub struct RopeNode {
    pub storage: RopeStorage,
    pub metrics: RopeMetrics,
}

pub type RopeHandle = Arc<RopeNode>;

/// The shared empty leaf. `Arc` lets this be a plain process-wide static; the
/// rope is immutable, so one instance is valid for every document.
static EMPTY_NODE: std::sync::OnceLock<RopeHandle> = std::sync::OnceLock::new();

pub fn empty_node() -> RopeHandle {
    EMPTY_NODE
        .get_or_init(|| Arc::new(RopeNode::with_leaf(Arc::new(RopeLeaf::new("")))))
        .clone()
}

impl RopeNode {
    pub const MAX_LEAF_BYTES: usize = 1024;

    pub fn with_leaf(leaf: Arc<RopeLeaf>) -> Self {
        let height = if leaf.bytes.is_empty() { 0 } else { 1 };
        let metrics = RopeMetrics {
            utf8_length: leaf.bytes.len(),
            character_count: leaf.character_count(),
            newline_count: leaf.newline_characters.len(),
            line_feed_count: leaf.line_feed_bytes.len(),
            utf16_length: leaf.utf16_length,
            height,
        };
        Self {
            storage: RopeStorage::Leaf(leaf),
            metrics,
        }
    }

    pub fn new(left: RopeHandle, right: RopeHandle) -> Self {
        let metrics = RopeMetrics {
            utf8_length: left.metrics.utf8_length + right.metrics.utf8_length,
            character_count: left.metrics.character_count + right.metrics.character_count,
            newline_count: left.metrics.newline_count + right.metrics.newline_count,
            line_feed_count: left.metrics.line_feed_count + right.metrics.line_feed_count,
            utf16_length: left.metrics.utf16_length + right.metrics.utf16_length,
            height: left.metrics.height.max(right.metrics.height) + 1,
        };
        Self {
            storage: RopeStorage::Branch(RopeBranch { left, right }),
            metrics,
        }
    }

    pub fn first_leaf(&self) -> Arc<RopeLeaf> {
        match &self.storage {
            RopeStorage::Leaf(leaf) => leaf.clone(),
            RopeStorage::Branch(branch) => branch.left.first_leaf(),
        }
    }

    pub fn last_leaf(&self) -> Arc<RopeLeaf> {
        match &self.storage {
            RopeStorage::Leaf(leaf) => leaf.clone(),
            RopeStorage::Branch(branch) => branch.right.last_leaf(),
        }
    }

    pub fn build(string: &str) -> RopeHandle {
        let mut leaves: Vec<RopeHandle> = Vec::new();
        let mut text = String::new();
        let mut byte_count = 0usize;
        // Cut only at extended grapheme boundaries; a single oversized cluster
        // is kept whole even when it exceeds the packing target.
        for grapheme in string.graphemes(true) {
            let size = grapheme.len();
            if byte_count > 0 && byte_count + size > Self::MAX_LEAF_BYTES {
                leaves.push(Arc::new(RopeNode::with_leaf(Arc::new(RopeLeaf::new(
                    std::mem::take(&mut text),
                )))));
                byte_count = 0;
            }
            text.push_str(grapheme);
            byte_count += size;
        }
        if !text.is_empty() {
            leaves.push(Arc::new(RopeNode::with_leaf(Arc::new(RopeLeaf::new(text)))));
        }
        Self::balanced(&leaves)
    }

    fn balanced(leaves: &[RopeHandle]) -> RopeHandle {
        if leaves.is_empty() {
            return empty_node();
        }
        if leaves.len() == 1 {
            return leaves[0].clone();
        }
        let middle = leaves.len() / 2;
        Arc::new(RopeNode::new(
            Self::balanced(&leaves[..middle]),
            Self::balanced(&leaves[middle..]),
        ))
    }
}

/// A mutable handle used by editing sessions that need to swap roots cheaply.
/// `RefCell` keeps this UI-thread-only; the *buffer value* it produces is
/// shareable, but the live editing session is not.
pub type SharedRope = Rc<RefCell<RopeHandle>>;

pub fn shared(root: RopeHandle) -> SharedRope {
    Rc::new(RefCell::new(root))
}
