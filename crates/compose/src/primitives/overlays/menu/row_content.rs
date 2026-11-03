//! Translation of Primitives/Overlays/Menu/RowContent.swift.
//!
//! Optional presentation for one menu row. `MenuItem::title` stays the
//! accessible name and the type-ahead key, so rich titles never change how a row
//! is found or announced.
use cove_core::Rect;
use cove_core::scene::Point;
use cove_imaging::BundleImageResource;

use crate::primitives::text::rich::run::TextRun;

/// Optional presentation for one menu row.
#[derive(Clone, Default)]
pub struct MenuItemContent {
    /// Leading glyph. Any icon in a menu level reserves one shared lane.
    pub icon: Option<BundleImageResource>,
    /// Styled visual title. Run links are ignored: the row is the only target.
    pub rich_title: Vec<TextRun>,
    /// Secondary line under the title.
    pub detail: Option<String>,
    /// Marks the row as a link to the destination. Activation still runs the
    /// item's action; assistive technology hears a link and a trailing glyph
    /// distinguishes it from commands.
    pub link: Option<String>,
}

impl MenuItemContent {
    pub fn new(
        icon: Option<BundleImageResource>,
        rich_title: Vec<TextRun>,
        detail: Option<String>,
        link: Option<String>,
    ) -> Self {
        Self {
            icon,
            rich_title: rich_title.into_iter().map(|mut run| { run.link = None; run }).collect(),
            detail: detail.filter(|value| !value.is_empty()),
            link: link.filter(|value| !value.is_empty()),
        }
    }
}

/// Pointer travel toward an open submenu. A row crossed on the way keeps the
/// submenu open longer (a safe triangle from the previous pointer position to
/// the submenu's near edge), so diagonal moves do not switch branches.
#[derive(Clone, Debug, Default)]
pub struct MenuPointerIntent {
    previous: Option<Point>,
    current: Option<Point>,
    pub submenu_frame: Option<Rect>,
}

impl MenuPointerIntent {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, point: Point) {
        if self.current == Some(point) {
            return;
        }
        self.previous = self.current;
        self.current = Some(point);
    }

    pub fn is_heading_to_submenu(&self) -> bool {
        let (Some(apex), Some(point), Some(frame)) =
            (self.previous, self.current, self.submenu_frame)
        else {
            return false;
        };
        if frame.width <= 0.0 || frame.height <= 0.0 {
            return false;
        }
        let edge = if point.x <= frame.x {
            frame.x
        } else if point.x >= frame.x + frame.width {
            frame.x + frame.width
        } else {
            return contains(frame, point);
        };
        let slack = 4.0;
        triangle_contains(
            apex,
            Point::new(edge, frame.y - slack),
            Point::new(edge, frame.y + frame.height + slack),
            point,
        )
    }

    pub fn reset(&mut self) {
        self.previous = None;
        self.current = None;
        self.submenu_frame = None;
    }
}

fn contains(rect: Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

/// Half-plane test: the point is inside when it is not strictly on one side of
/// every edge.
pub fn triangle_contains(a: Point, b: Point, c: Point, point: Point) -> bool {
    fn sign(p1: Point, p2: Point, p3: Point) -> f32 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    }
    let d1 = sign(point, a, b);
    let d2 = sign(point, b, c);
    let d3 = sign(point, c, a);
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}
