//! Danmaku (bullet comments) over a video.
//!
//! Every comment's place on screen is a pure function of the media position,
//! so comments freeze on pause, follow seeks, and speed up with the playback
//! rate. Lanes are assigned once, when a comment becomes due, and never move.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use cove_composition::{AnyView, PrimitiveView, View};
use cove_core::scene::{Edge, LayoutNode, Node, NodeResource, PositionType};
use cove_core::{Color, Font, FontWeight, Point, Signal, Size};
use cove_media::MediaTime;

use super::frame_ticker::FrameTicker;
use crate::modifiers::style::AppearanceExt;
use crate::primitives::text::Text;
use crate::{Scope, impl_primitive_view};

/// Seconds a top or bottom comment stays on screen.
const PINNED_SECONDS: f64 = 4.0;
/// Horizontal clearance kept between two comments in one lane.
const LANE_GAP: f32 = 12.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DanmakuMode {
    #[default]
    Scroll,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DanmakuComment {
    /// Assigned by the feed; unique within it.
    pub id: u64,
    pub time: MediaTime,
    pub text: String,
    pub color: Color,
    pub mode: DanmakuMode,
    /// Comments the viewer sent are outlined.
    pub is_own: bool,
}

impl DanmakuComment {
    pub fn new(time: MediaTime, text: impl Into<String>) -> Self {
        Self {
            id: 0,
            time,
            text: text.into(),
            color: Color::WHITE,
            mode: DanmakuMode::Scroll,
            is_own: false,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn mode(mut self, mode: DanmakuMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn own(mut self, is_own: bool) -> Self {
        self.is_own = is_own;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DanmakuSettings {
    pub enabled: bool,
    /// `0.1..=1`.
    pub opacity: f32,
    /// Seconds a scrolling comment takes to cross the video.
    pub scroll_seconds: f32,
    /// Multiplies the size derived from the video height.
    pub font_scale: f32,
    /// Fraction of the video height comments may use, from the top.
    pub area: f32,
}

impl Default for DanmakuSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            opacity: 0.9,
            scroll_seconds: 8.0,
            font_scale: 1.0,
            area: 0.75,
        }
    }
}

impl DanmakuSettings {
    fn sanitized(self) -> Self {
        let finite = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
        Self {
            enabled: self.enabled,
            opacity: finite(self.opacity, 0.9).clamp(0.1, 1.0),
            scroll_seconds: finite(self.scroll_seconds, 8.0).clamp(3.0, 20.0),
            font_scale: finite(self.font_scale, 1.0).clamp(0.5, 2.0),
            area: finite(self.area, 0.75).clamp(0.1, 1.0),
        }
    }

    /// Font size for a video of `height` points.
    pub fn font_size(&self, height: f32) -> f32 {
        (height / 15.0).clamp(13.0, 30.0) * self.sanitized().font_scale
    }
}

#[derive(Default)]
struct FeedInner {
    comments: Vec<DanmakuComment>,
    next_id: u64,
    revision: u64,
}

/// Comments sorted by time. Clones share one list.
#[derive(Clone, Default)]
pub struct DanmakuFeed(Rc<RefCell<FeedInner>>);

impl DanmakuFeed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_comments(comments: impl IntoIterator<Item = DanmakuComment>) -> Self {
        let feed = Self::new();
        feed.extend(comments);
        feed
    }

    /// Adds a comment and returns its id.
    pub fn push(&self, comment: DanmakuComment) -> u64 {
        let mut inner = self.0.borrow_mut();
        inner.next_id += 1;
        let id = inner.next_id;
        let comment = DanmakuComment { id, ..comment };
        let at = inner
            .comments
            .partition_point(|existing| existing.time <= comment.time);
        inner.comments.insert(at, comment);
        inner.revision += 1;
        id
    }

    pub fn extend(&self, comments: impl IntoIterator<Item = DanmakuComment>) {
        for comment in comments {
            self.push(comment);
        }
    }

    pub fn clear(&self) {
        let mut inner = self.0.borrow_mut();
        inner.comments.clear();
        inner.revision += 1;
    }

    pub fn len(&self) -> usize {
        self.0.borrow().comments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn revision(&self) -> u64 {
        self.0.borrow().revision
    }

    pub fn comments(&self) -> Vec<DanmakuComment> {
        self.0.borrow().comments.clone()
    }

    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    fn first_at_or_after(&self, seconds: f64) -> usize {
        self.0
            .borrow()
            .comments
            .partition_point(|comment| comment.time.seconds() < seconds)
    }

    fn get(&self, index: usize) -> Option<DanmakuComment> {
        self.0.borrow().comments.get(index).cloned()
    }

    /// Imports Bilibili's XML danmaku format (`<d p="time,mode,size,color,...">text</d>`).
    /// Modes 1–3 scroll, 4 is bottom and 5 is top; others are skipped.
    pub fn parse_bilibili_xml(xml: &str) -> Vec<DanmakuComment> {
        let mut comments = Vec::new();
        let mut rest = xml;
        while let Some(start) = rest.find("<d ") {
            rest = &rest[start + 3..];
            let Some(attributes_end) = rest.find('>') else {
                break;
            };
            let attributes = &rest[..attributes_end];
            rest = &rest[attributes_end + 1..];
            let Some(close) = rest.find("</d>") else {
                break;
            };
            let text = unescape(&rest[..close]);
            rest = &rest[close + 4..];
            let Some(p) = attributes
                .split("p=\"")
                .nth(1)
                .and_then(|value| value.split('"').next())
            else {
                continue;
            };
            let fields: Vec<&str> = p.split(',').collect();
            let (Some(time), Some(mode)) = (
                fields.first().and_then(|value| value.parse::<f64>().ok()),
                fields.get(1).and_then(|value| value.parse::<u32>().ok()),
            ) else {
                continue;
            };
            let mode = match mode {
                1..=3 => DanmakuMode::Scroll,
                4 => DanmakuMode::Bottom,
                5 => DanmakuMode::Top,
                _ => continue,
            };
            let rgb = fields
                .get(3)
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0xFF_FF_FF);
            let channel = |shift: u32| ((rgb >> shift) & 0xFF) as u8;
            let Ok(time) = MediaTime::from_seconds(time.max(0.0)) else {
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            comments.push(
                DanmakuComment::new(time, text)
                    .mode(mode)
                    .color(Color::from_rgba8(channel(16), channel(8), channel(0), 255)),
            );
        }
        comments
    }
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// A comment on screen and the lane it was given.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveDanmaku {
    pub comment: DanmakuComment,
    pub lane: usize,
    /// Width estimated from the text; layout supplies the measured width.
    pub estimated_width: f32,
}

/// Decides which comments are on screen and where. Pure: no UI state.
#[derive(Default)]
pub struct DanmakuScheduler {
    cursor: usize,
    last_position: Option<f64>,
    feed_revision: u64,
    active: Vec<ActiveDanmaku>,
}

impl DanmakuScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> &[ActiveDanmaku] {
        &self.active
    }

    /// Advances to `position` seconds. Returns whether the on-screen set changed.
    pub fn advance(
        &mut self,
        feed: &DanmakuFeed,
        position: f64,
        viewport: Size,
        settings: &DanmakuSettings,
    ) -> bool {
        let settings = settings.sanitized();
        let before = self.active.len();
        let mut changed = false;
        let jumped = self
            .last_position
            .is_none_or(|last| position < last - 0.05 || position > last + 1.5);
        if jumped {
            changed |= !self.active.is_empty();
            self.active.clear();
            self.cursor = feed.first_at_or_after(position);
        } else if feed.revision() != self.feed_revision {
            let last = self.last_position.unwrap_or(position);
            self.cursor = feed.first_at_or_after(last - 0.001);
        }
        self.feed_revision = feed.revision();
        self.last_position = Some(position);
        if !settings.enabled || viewport.width <= 0.0 || viewport.height <= 0.0 {
            changed |= !self.active.is_empty();
            self.active.clear();
            return changed;
        }
        let scroll = f64::from(settings.scroll_seconds);
        self.active.retain(|item| {
            let elapsed = position - item.comment.time.seconds();
            let lifetime = match item.comment.mode {
                DanmakuMode::Scroll => scroll,
                DanmakuMode::Top | DanmakuMode::Bottom => PINNED_SECONDS,
            };
            (0.0..=lifetime).contains(&elapsed)
        });
        changed |= self.active.len() != before;
        let font = settings.font_size(viewport.height);
        let lane_height = lane_height(font);
        let lanes = ((viewport.height * settings.area) / lane_height)
            .floor()
            .max(1.0) as usize;
        while let Some(comment) = feed.get(self.cursor) {
            if comment.time.seconds() > position {
                break;
            }
            self.cursor += 1;
            if self.active.iter().any(|item| item.comment.id == comment.id) {
                continue;
            }
            let estimated_width = estimate_width(&comment.text, font);
            if let Some(lane) =
                self.free_lane(&comment, estimated_width, position, viewport, lanes, scroll)
            {
                self.active.push(ActiveDanmaku {
                    comment,
                    lane,
                    estimated_width,
                });
                changed = true;
            }
        }
        changed
    }

    fn free_lane(
        &self,
        comment: &DanmakuComment,
        width: f32,
        position: f64,
        viewport: Size,
        lanes: usize,
        scroll: f64,
    ) -> Option<usize> {
        (0..lanes).find(|lane| {
            self.active
                .iter()
                .filter(|item| item.lane == *lane && item.comment.mode == comment.mode)
                .all(|item| match comment.mode {
                    DanmakuMode::Top | DanmakuMode::Bottom => false,
                    DanmakuMode::Scroll => {
                        let travel = f64::from(viewport.width + item.estimated_width);
                        let progress = (position - item.comment.time.seconds()) / scroll;
                        let entered =
                            progress * travel >= f64::from(item.estimated_width + LANE_GAP);
                        // A longer (faster) comment must not reach the earlier one's tail.
                        let ours = f64::from(viewport.width) / f64::from(viewport.width + width);
                        entered && ours >= 1.0 - progress
                    }
                })
        })
    }

    /// Top-left corner of `item` at `position` for its measured `width`.
    pub fn placement(
        item: &ActiveDanmaku,
        position: f64,
        viewport: Size,
        settings: &DanmakuSettings,
        width: f32,
    ) -> Point {
        let settings = settings.sanitized();
        let lane_height = lane_height(settings.font_size(viewport.height));
        let lane = item.lane as f32 * lane_height + 4.0;
        let elapsed = (position - item.comment.time.seconds()).max(0.0);
        match item.comment.mode {
            DanmakuMode::Scroll => {
                let progress = (elapsed / f64::from(settings.scroll_seconds)) as f32;
                Point::new(viewport.width - progress * (viewport.width + width), lane)
            }
            DanmakuMode::Top => Point::new((viewport.width - width) / 2.0, lane),
            DanmakuMode::Bottom => Point::new(
                (viewport.width - width) / 2.0,
                viewport.height - lane - lane_height - 40.0,
            ),
        }
    }
}

fn lane_height(font: f32) -> f32 {
    (font * 1.3).ceil() + 2.0
}

/// Wide (CJK, emoji) characters take about one em, others about half.
fn estimate_width(text: &str, font: f32) -> f32 {
    text.chars()
        .map(|character| if (character as u32) > 0x2E7F { 1.0 } else { 0.56 })
        .sum::<f32>()
        * font
}

/// Media position extrapolated between snapshot reports.
#[derive(Default)]
struct PlaybackClock {
    anchor: Cell<(f64, Option<Instant>)>,
    playing: Cell<bool>,
    rate: Cell<f64>,
}

impl PlaybackClock {
    fn sync(&self, position: f64, playing: bool, rate: f64) {
        let current = self.now();
        // Small report jitter must not shake the comments; real jumps re-anchor.
        if !playing || !self.playing.get() || (current - position).abs() > 0.25 {
            self.anchor.set((position, playing.then(Instant::now)));
        }
        self.playing.set(playing);
        self.rate.set(if rate.is_finite() { rate } else { 1.0 });
    }

    fn now(&self) -> f64 {
        let (position, since) = self.anchor.get();
        match since {
            Some(since) if self.playing.get() => {
                position + since.elapsed().as_secs_f64() * self.rate.get()
            }
            _ => position,
        }
    }
}

/// Comments over a video, driven by the playback position.
#[derive(Clone)]
pub struct DanmakuLayer {
    pub feed: DanmakuFeed,
    pub settings: DanmakuSettings,
    pub position: f64,
    pub playing: bool,
    pub rate: f64,
}

impl DanmakuLayer {
    pub fn new(feed: DanmakuFeed, position: f64) -> Self {
        Self {
            feed,
            settings: DanmakuSettings::default(),
            position,
            playing: false,
            rate: 1.0,
        }
    }

    pub fn settings(mut self, settings: DanmakuSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn playing(mut self, playing: bool, rate: f64) -> Self {
        self.playing = playing;
        self.rate = rate;
        self
    }
}

impl View for DanmakuLayer {
    fn body(&self, cx: &Scope) -> AnyView {
        let active = cx.state(Vec::<ActiveDanmaku>::new);
        let scheduler = cx.remember(|| Rc::new(RefCell::new(DanmakuScheduler::new())));
        let clock = cx.remember(|| Rc::new(PlaybackClock::default()));
        clock.sync(self.position, self.playing, self.rate);
        AnyView::new(DanmakuHost {
            layer: self.clone(),
            items: active.get(),
            active,
            scheduler,
            clock,
        })
    }
}

#[derive(Clone)]
struct DanmakuHost {
    layer: DanmakuLayer,
    items: Vec<ActiveDanmaku>,
    active: Signal<Vec<ActiveDanmaku>>,
    scheduler: Rc<RefCell<DanmakuScheduler>>,
    clock: Rc<PlaybackClock>,
}

impl DanmakuHost {
    /// Advances the schedule and moves every comment node to its place.
    fn refresh(&self, node: &Node) {
        let frame = node.frame();
        let viewport = Size {
            width: frame.width,
            height: frame.height,
        };
        let position = self.clock.now();
        let changed = self.scheduler.borrow_mut().advance(
            &self.layer.feed,
            position,
            viewport,
            &self.layer.settings,
        );
        if changed {
            self.active
                .set(self.scheduler.borrow().active().to_vec());
        }
        let items = self.active.get_untracked();
        for (item, child) in items.iter().zip(node.children()) {
            let measured = child.frame().width;
            let width = if measured > 0.0 {
                measured
            } else {
                item.estimated_width
            };
            let at = DanmakuScheduler::placement(
                item,
                position,
                viewport,
                &self.layer.settings,
                width,
            );
            if let Some(layout) = child.layout_node() {
                layout.set_position(at.x.round(), Edge::Left);
                layout.set_position(at.y.round(), Edge::Top);
            }
        }
    }
}

impl PrimitiveView for DanmakuHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_clips_to_bounds(true);
        let ticker: Rc<dyn NodeResource> = FrameTicker::new();
        node.add_resource(ticker);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        node.set_opacity(self.layer.settings.sanitized().opacity);
        if let Some(ticker) = node.first_resource::<FrameTicker>() {
            let host = self.clone();
            ticker.configure(
                self.layer.playing && self.layer.settings.enabled,
                Rc::new(move |node| host.refresh(node)),
            );
        }
        self.refresh(node);
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let height = node.frame().height;
        let font = self.layer.settings.font_size(if height > 0.0 { height } else { 270.0 });
        self.items
            .iter()
            .map(|item| AnyView::new(DanmakuItemView::new(&item.comment, font)))
            .collect()
    }
}
impl_primitive_view!(DanmakuHost);

/// One comment: coloured text over a dark offset copy for legibility.
#[derive(Clone)]
struct DanmakuItemView {
    text: String,
    color: Color,
    is_own: bool,
    font: f32,
}

impl DanmakuItemView {
    fn new(comment: &DanmakuComment, font: f32) -> Self {
        Self {
            text: comment.text.clone(),
            color: comment.color,
            is_own: comment.is_own,
            font,
        }
    }
}

impl PrimitiveView for DanmakuItemView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(f32::MAX / 4.0, Edge::Left);
        layout.set_position(0.0, Edge::Top);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        node.set_border_color(self.is_own.then(|| Color::WHITE.multiplied_alpha(0.9)));
        node.set_border_width(if self.is_own { 1.0 } else { 0.0 });
        node.set_corner_radius(if self.is_own { 3.0 } else { 0.0 });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let font = Font::system(self.font, FontWeight::Semibold);
        let shadow = Text::new(self.text.clone())
            .color(Color::BLACK.multiplied_alpha(0.75))
            .line_limit(Some(1))
            .font(font.clone());
        let label = Text::new(self.text.clone())
            .color(self.color)
            .line_limit(Some(1))
            .font(font);
        vec![
            AnyView::new(ShadowSlot(AnyView::new(shadow))),
            AnyView::new(label),
        ]
    }
}
impl_primitive_view!(DanmakuItemView);

/// Places the shadow copy one point down and right, outside the flow.
#[derive(Clone)]
struct ShadowSlot(AnyView);

impl PrimitiveView for ShadowSlot {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(1.5, Edge::Left);
        layout.set_position(1.5, Edge::Top);
        Some(layout)
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.0.clone()]
    }
}
impl_primitive_view!(ShadowSlot);

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: f64) -> MediaTime {
        MediaTime::from_seconds(seconds).expect("time")
    }

    const VIEW: Size = Size {
        width: 640.0,
        height: 360.0,
    };

    #[test]
    fn comments_appear_scroll_and_expire_with_media_time() {
        let feed = DanmakuFeed::from_comments([
            DanmakuComment::new(at(1.0), "first"),
            DanmakuComment::new(at(1.0), "second"),
            DanmakuComment::new(at(2.0), "top").mode(DanmakuMode::Top),
        ]);
        let settings = DanmakuSettings::default();
        let mut scheduler = DanmakuScheduler::new();
        assert!(!scheduler.advance(&feed, 0.5, VIEW, &settings));
        assert!(scheduler.advance(&feed, 1.01, VIEW, &settings));
        let lanes: Vec<usize> = scheduler.active().iter().map(|item| item.lane).collect();
        assert_eq!(lanes, vec![0, 1], "simultaneous comments take separate lanes");
        let start = DanmakuScheduler::placement(&scheduler.active()[0], 1.0, VIEW, &settings, 50.0);
        let later = DanmakuScheduler::placement(&scheduler.active()[0], 5.0, VIEW, &settings, 50.0);
        assert_eq!(start.x, 640.0);
        assert!(later.x < start.x);
        let mut play_to = |scheduler: &mut DanmakuScheduler, from: f64, to: f64| {
            let mut position = from;
            while position < to {
                position = (position + 0.25).min(to);
                scheduler.advance(&feed, position, VIEW, &settings);
            }
        };
        play_to(&mut scheduler, 1.01, 2.0);
        assert_eq!(scheduler.active().len(), 3);
        play_to(&mut scheduler, 2.0, 6.5);
        assert_eq!(scheduler.active().len(), 2, "the top comment expires after 4 s");
        play_to(&mut scheduler, 6.5, 9.5);
        assert!(scheduler.active().is_empty());
    }

    #[test]
    fn seeking_resets_and_skips_past_comments() {
        let feed = DanmakuFeed::from_comments(
            (0..20).map(|i| DanmakuComment::new(at(f64::from(i)), format!("c{i}"))),
        );
        let settings = DanmakuSettings::default();
        let mut scheduler = DanmakuScheduler::new();
        scheduler.advance(&feed, 3.0, VIEW, &settings);
        scheduler.advance(&feed, 15.0, VIEW, &settings);
        assert!(
            scheduler
                .active()
                .iter()
                .all(|item| item.comment.time.seconds() >= 15.0),
            "a jump does not flood the screen with skipped comments"
        );
        let id = feed.push(DanmakuComment::new(at(15.0), "sent").own(true));
        scheduler.advance(&feed, 15.02, VIEW, &settings);
        assert!(scheduler.active().iter().any(|item| item.comment.id == id));
    }

    #[test]
    fn a_full_area_drops_instead_of_overlapping() {
        let feed = DanmakuFeed::from_comments(
            (0..100).map(|i| DanmakuComment::new(at(1.0), format!("comment number {i}"))),
        );
        let settings = DanmakuSettings {
            area: 0.25,
            ..DanmakuSettings::default()
        };
        let mut scheduler = DanmakuScheduler::new();
        scheduler.advance(&feed, 1.0, VIEW, &settings);
        let count = scheduler.active().len();
        assert!(count > 0 && count < 10, "{count}");
        let mut lanes: Vec<usize> = scheduler.active().iter().map(|item| item.lane).collect();
        lanes.dedup();
        assert_eq!(lanes.len(), count);
    }

    #[test]
    fn bilibili_xml_is_imported() {
        let xml = r#"<i><d p="1.5,1,25,16711680,0,0,0,0">red &amp; scroll</d><d p="3,5,25,65280">top</d><d p="4,4,25,255">bottom</d><d p="2,7,25,1">skip</d></i>"#;
        let comments = DanmakuFeed::parse_bilibili_xml(xml);
        assert_eq!(comments.len(), 3);
        assert_eq!(comments[0].text, "red & scroll");
        assert_eq!(comments[0].color, Color::from_rgba8(255, 0, 0, 255));
        assert_eq!(comments[1].mode, DanmakuMode::Top);
        assert_eq!(comments[2].mode, DanmakuMode::Bottom);
        assert_eq!(comments[0].time, at(1.5));
    }
}
