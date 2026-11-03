//! Inline presentation traits and one styled span of a rich paragraph.

use std::ops::{BitOr, BitOrAssign};

/// Inline presentation traits of one `TextRun`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextTraits(u8);

impl TextTraits {
    pub const BOLD: Self = Self(1 << 0);
    pub const ITALIC: Self = Self(1 << 1);
    /// Monospaced type on a tinted chip, as for inline code.
    pub const CODE: Self = Self(1 << 2);
    pub const STRIKETHROUGH: Self = Self(1 << 3);

    pub const fn from_raw(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn raw_value(self) -> u8 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for TextTraits {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for TextTraits {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// One styled span of a `Text` paragraph. Runs wrap together as a single
/// paragraph; a `"\n"` inside a run is a hard line break.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextRun {
    pub text: String,
    pub traits: TextTraits,
    /// The destination handed to `Text`'s link handler. Adjacent runs with
    /// the same destination form one link.
    pub link: Option<String>,
}

impl TextRun {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            traits: TextTraits::default(),
            link: None,
        }
    }

    pub fn with_traits(text: impl Into<String>, traits: TextTraits) -> Self {
        Self {
            text: text.into(),
            traits,
            link: None,
        }
    }

    pub fn link(mut self, destination: impl Into<String>) -> Self {
        self.link = Some(destination.into());
        self
    }
}
