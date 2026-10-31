//! Translation of CoveUICore/Bidi.swift. Resolution and L2 reordering use shared C kernels.
use std::ops::Range;
use unicode_general_category::{GeneralCategory as Category, get_general_category};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidiClass {
    L,
    R,
    Al,
    En,
    Es,
    Et,
    An,
    Cs,
    Nsm,
    Bn,
    B,
    S,
    Ws,
    On,
}
impl BidiClass {
    pub fn of(scalar: char) -> Self {
        use BidiClass::*;
        let value = scalar as u32;
        match value {
            0x0a | 0x0d | 0x1c..=0x1e | 0x85 | 0x2029 => return B,
            0x09 | 0x0b | 0x1f => return S,
            0x0c | 0x20 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x205f | 0x3000 => return Ws,
            0x30..=0x39
            | 0xb2
            | 0xb3
            | 0xb9
            | 0x06f0..=0x06f9
            | 0x2070
            | 0x2074..=0x2079
            | 0x2080..=0x2089
            | 0x2488..=0x249b
            | 0xff10..=0xff19
            | 0x1d7ce..=0x1d7ff => return En,
            0x0600..=0x0605
            | 0x0660..=0x0669
            | 0x066b
            | 0x066c
            | 0x06dd
            | 0x0890
            | 0x0891
            | 0x08e2
            | 0x10e60..=0x10e7e => return An,
            0x2b | 0x2d | 0x207a | 0x207b | 0x208a | 0x208b | 0x2212 | 0xfb29 | 0xfe62 | 0xfe63
            | 0xff0b | 0xff0d => return Es,
            0x23..=0x25
            | 0xa2..=0xa5
            | 0xb0
            | 0xb1
            | 0x058f
            | 0x0609
            | 0x060a
            | 0x066a
            | 0x09f2
            | 0x09f3
            | 0x0e3f
            | 0x17db
            | 0x2030..=0x2034
            | 0x20a0..=0x20cf
            | 0x212e
            | 0x2213
            | 0xfe5f
            | 0xfe69
            | 0xfe6a
            | 0xff03..=0xff05
            | 0xffe0
            | 0xffe1
            | 0xffe5
            | 0xffe6 => return Et,
            0x2c | 0x2e | 0x2f | 0x3a | 0xa0 | 0x060c | 0x202f | 0x2044 | 0xfe50 | 0xfe52
            | 0xfe55 | 0xff0c | 0xff0e | 0xff0f | 0xff1a => return Cs,
            0x200e => return L,
            0x200f => return R,
            0x061c => return Al,
            0x202a..=0x202e
            | 0x2066..=0x2069
            | 0x200b..=0x200d
            | 0x2060..=0x2064
            | 0xfeff
            | 0xad => return Bn,
            _ => {}
        }
        let category = get_general_category(scalar);
        match category {
            Category::NonspacingMark | Category::EnclosingMark => return Nsm,
            Category::Format | Category::Control => return Bn,
            _ => {}
        }
        match value {
            0x0590..=0x05ff
            | 0x07c0..=0x085f
            | 0xfb1d..=0xfb4f
            | 0x10800..=0x10cff
            | 0x10d40..=0x10ebf
            | 0x10f00..=0x10f2f
            | 0x10f70..=0x10fff
            | 0x1e800..=0x1edff
            | 0x1ef00..=0x1efff => R,
            0x0600..=0x07bf
            | 0x0860..=0x08ff
            | 0xfb50..=0xfdcf
            | 0xfdf0..=0xfdff
            | 0xfe70..=0xfeff
            | 0x10d00..=0x10d3f
            | 0x10ec0..=0x10eff
            | 0x10f30..=0x10f6f
            | 0x1ee00..=0x1eeff => Al,
            _ => match category {
                Category::UppercaseLetter
                | Category::LowercaseLetter
                | Category::TitlecaseLetter
                | Category::ModifierLetter
                | Category::OtherLetter
                | Category::SpacingMark
                | Category::DecimalNumber
                | Category::LetterNumber
                | Category::OtherNumber
                | Category::PrivateUse => L,
                _ => On,
            },
        }
    }
    pub fn is_strong_right_to_left(self) -> bool {
        matches!(self, Self::R | Self::Al)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidiParagraph {
    pub range: Range<usize>,
    pub base_level: u8,
}
impl BidiParagraph {
    pub fn is_right_to_left(&self) -> bool {
        self.base_level & 1 == 1
    }
}
#[derive(Clone, Debug)]
pub struct BidiParagraphs {
    pub levels: Vec<u8>,
    pub paragraphs: Vec<BidiParagraph>,
    classes: Vec<BidiClass>,
}
unsafe extern "C" {
    fn cove_bidi_resolve(
        source: *const u8,
        scratch: *mut u8,
        count: usize,
        base: u8,
        levels: *mut u8,
    );
    fn cove_bidi_visual_order(levels: *const u8, count: usize, order: *mut usize);
}
impl BidiParagraphs {
    pub fn contains_right_to_left(text: &str) -> bool {
        text.chars().any(|scalar| {
            scalar as u32 >= 0x0590
                && matches!(
                    BidiClass::of(scalar),
                    BidiClass::R | BidiClass::Al | BidiClass::An
                )
        })
    }
    pub fn new(text: &str, default_right_to_left: bool) -> Self {
        let scalars: Vec<_> = text
            .chars()
            .map(|c| (BidiClass::of(c), c.len_utf8()))
            .collect();
        let mut result = Self {
            levels: Vec::with_capacity(text.len()),
            classes: Vec::with_capacity(text.len()),
            paragraphs: vec![],
        };
        let mut start = 0;
        for end in 1..=scalars.len() {
            if scalars[end - 1].0 == BidiClass::B || end == scalars.len() {
                result.resolve_paragraph(&scalars[start..end], default_right_to_left);
                start = end;
            }
        }
        if scalars.is_empty() {
            result.resolve_paragraph(&[], default_right_to_left);
        }
        result
    }
    fn resolve_paragraph(&mut self, scalars: &[(BidiClass, usize)], default_rtl: bool) {
        let base = scalars
            .iter()
            .find_map(|(class, _)| {
                if *class == BidiClass::L {
                    Some(0)
                } else if class.is_strong_right_to_left() {
                    Some(1)
                } else {
                    None
                }
            })
            .unwrap_or(u8::from(default_rtl));
        let source: Vec<u8> = scalars.iter().map(|(class, _)| *class as u8).collect();
        let mut scratch = vec![0; source.len()];
        let mut levels = vec![0; source.len()];
        // SAFETY: all buffers have scalar count bytes and do not overlap. The C enum order matches repr(u8) above.
        unsafe {
            cove_bidi_resolve(
                source.as_ptr(),
                scratch.as_mut_ptr(),
                source.len(),
                base,
                levels.as_mut_ptr(),
            );
        }
        let byte_start = self.levels.len();
        for (index, &(class, bytes)) in scalars.iter().enumerate() {
            self.levels
                .extend(std::iter::repeat_n(levels[index], bytes));
            self.classes.extend(std::iter::repeat_n(class, bytes));
        }
        self.paragraphs.push(BidiParagraph {
            range: byte_start..self.levels.len(),
            base_level: base,
        });
    }
    pub fn is_mixed(&self) -> bool {
        self.levels.iter().any(|&l| l != 0)
            || self.paragraphs.iter().any(BidiParagraph::is_right_to_left)
    }
    pub fn level_at_byte(&self, offset: usize) -> u8 {
        self.levels
            .get(offset)
            .copied()
            .unwrap_or_else(|| self.paragraph_at_byte(offset).base_level)
    }
    pub fn paragraph_at_byte(&self, offset: usize) -> &BidiParagraph {
        self.paragraphs
            .iter()
            .find(|p| p.range.contains(&offset))
            .unwrap_or_else(|| {
                self.paragraphs
                    .last()
                    .expect("paragraph list always contains the empty paragraph")
            })
    }
    pub fn is_trailing_whitespace(&self, offset: usize) -> bool {
        self.classes.get(offset).is_some_and(|c| {
            matches!(
                c,
                BidiClass::Ws | BidiClass::S | BidiClass::B | BidiClass::Bn
            )
        })
    }
    pub fn level_runs(&self, range: Range<usize>) -> Vec<(Range<usize>, u8)> {
        assert!(range.start <= range.end && range.end <= self.levels.len());
        let mut result = vec![];
        let mut start = range.start;
        for offset in range.clone().skip(1) {
            if self.levels[offset] != self.levels[offset - 1] {
                result.push((start..offset, self.levels[start]));
                start = offset;
            }
        }
        if start < range.end {
            result.push((start..range.end, self.levels[start]));
        }
        result
    }
    pub fn visual_order(levels: &[u8]) -> Vec<usize> {
        let mut order = vec![0; levels.len()];
        // SAFETY: C writes exactly levels.len() entries to a live usize array and only reads the input.
        unsafe {
            cove_bidi_visual_order(levels.as_ptr(), levels.len(), order.as_mut_ptr());
        }
        order
    }
}
