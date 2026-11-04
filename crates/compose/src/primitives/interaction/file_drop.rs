//! A drop area for files dragged in from outside the application.
//!
//! Two paths lead to the same callback: a platform drag enters through
//! [`crate::primitives::interaction::FileDropRegistry`], and a file chosen from
//! the native open panel enters through `FilePicker`. Touch and keyboard users
//! therefore reach the same control, which is why the zone ships its own browse
//! affordance instead of relying on a drag they cannot perform.
use crate::Scope;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use cove_composition::{AnyView, PrimitiveView, View};
use cove_core::scene::{Align, FlexDirection, LayoutNode, Node, NodeWeak};
use cove_core::{Binding, Point, Rect};

use crate::modifiers::interaction::AccessibilityExt;
use crate::primitives::controls::{Button, ButtonRole};
use crate::primitives::text::Text;
use crate::theme::theme_for_node;

/// One file the user dropped or picked.
///
/// `path` is the platform path as delivered; `name` is its final component, so a
/// control can show a readable label without re-parsing the path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedFile {
    pub path: String,
    pub name: String,
    pub byte_size: Option<u64>,
    pub kind: String,
}

impl DroppedFile {
    /// Describes a path without touching the filesystem.
    ///
    /// Size is left absent: a drop can arrive before anything is readable (a
    /// sandboxed URL, a removed file), and a control must not read every file
    /// just to paint a row.
    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        let name = file_name(&path);
        let kind = file_kind(&name);
        Self {
            path,
            name,
            byte_size: None,
            kind,
        }
    }

    pub fn byte_size(mut self, byte_size: u64) -> Self {
        self.byte_size = Some(byte_size);
        self
    }

    /// Whether this file matches one of `extensions` (case-insensitive, no dot).
    ///
    /// An empty list accepts every file.
    pub fn matches_any(&self, extensions: &HashSet<String>) -> bool {
        extensions.is_empty() || extensions.contains(&self.kind)
    }

    /// A human-readable size, or nothing when the size is unknown.
    pub fn size_text(&self) -> Option<String> {
        self.byte_size.map(format_bytes)
    }
}

/// The last path component, tolerating both separators.
fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}

/// The lowercase extension of `name`, or `""` when it has none.
fn file_kind(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| !extension.contains(['/', '\\']))
        .unwrap_or_default()
}

/// Formats a byte count with one decimal for the units above bytes.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// One registered drop zone.
struct Zone {
    node: NodeWeak,
    accepted_extensions: HashSet<String>,
    maximum_files: usize,
    is_enabled: bool,
    on_drop: Rc<dyn Fn(Vec<DroppedFile>)>,
    on_targeted: Option<Rc<dyn Fn(bool)>>,
    accepts: Option<Rc<dyn Fn(&[DroppedFile]) -> bool>>,
}

/// Holds every live file drop zone; the ambient instance is set by the host.
#[derive(Default)]
pub struct FileDropRegistry {
    zones: RefCell<Vec<Zone>>,
    targeted: RefCell<Option<NodeWeak>>,
}

impl FileDropRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &self,
        node: &Node,
        accepted_extensions: HashSet<String>,
        maximum_files: usize,
        is_enabled: bool,
        on_drop: Rc<dyn Fn(Vec<DroppedFile>)>,
        on_targeted: Option<Rc<dyn Fn(bool)>>,
        accepts: Option<Rc<dyn Fn(&[DroppedFile]) -> bool>>,
    ) {
        let mut zones = self.zones.borrow_mut();
        if let Some(existing) = zones
            .iter_mut()
            .find(|zone| Node::upgrade(&zone.node).is_some_and(|candidate| candidate == *node))
        {
            existing.accepted_extensions = accepted_extensions;
            existing.maximum_files = maximum_files;
            existing.is_enabled = is_enabled;
            existing.on_drop = on_drop;
            existing.on_targeted = on_targeted;
            existing.accepts = accepts;
            return;
        }
        zones.push(Zone {
            node: node.downgrade(),
            accepted_extensions,
            maximum_files,
            is_enabled,
            on_drop,
            on_targeted,
            accepts,
        });
    }

    pub fn unregister(&self, node: &Node) {
        self.zones
            .borrow_mut()
            .retain(|zone| Node::upgrade(&zone.node).is_none_or(|candidate| candidate != *node));
        let mut targeted = self.targeted.borrow_mut();
        if targeted
            .as_ref()
            .and_then(Node::upgrade)
            .is_some_and(|target| target == *node)
        {
            *targeted = None;
        }
    }

    /// Which zone the platform drag is currently over, if any.
    ///
    /// The platform reports the pointer while a drag is in flight; the registry
    /// answers so the zone can paint accept or reject chrome.
    pub fn update_position(&self, point: Point) -> Option<Node> {
        self.prune();
        let next = self.zone_at(point);
        let next_key = next.as_ref().map(|node| node.id().0);
        let previous = self.targeted.borrow().clone();
        let previous_key = previous
            .as_ref()
            .and_then(Node::upgrade)
            .map(|node| node.id().0);
        if previous_key == next_key {
            return next;
        }
        if let Some(previous) = previous.and_then(|node| Node::upgrade(&node))
            && let Some(on_targeted) = self
                .with_zone(&previous, |zone| zone.on_targeted.clone())
                .flatten()
        {
            on_targeted(false);
        }
        *self.targeted.borrow_mut() = next.as_ref().map(Node::downgrade);
        if let Some(node) = next.as_ref()
            && let Some(on_targeted) = self
                .with_zone(node, |zone| zone.on_targeted.clone())
                .flatten()
        {
            on_targeted(true);
        }
        next
    }

    /// Whether the zone under `point` would accept `files`.
    pub fn can_drop(&self, files: &[DroppedFile], point: Point) -> bool {
        self.zone_at(point).is_some_and(|node| {
            self.with_zone(&node, |zone| Self::accepts(files, zone))
                .unwrap_or(false)
        })
    }

    /// Delivers the files to the topmost zone under `point`.
    ///
    /// Returns whether a zone accepted the drop. A rejected drop is not an
    /// error: the type filter is a normal outcome and the caller keeps the
    /// previous state.
    pub fn drop(&self, files: &[DroppedFile], point: Point) -> bool {
        self.prune();
        let Some(node) = self.zone_at(point) else {
            self.clear_targeted();
            return false;
        };
        let accepted = self.with_zone(&node, |zone| {
            if !Self::accepts(files, zone) {
                return false;
            }
            (zone.on_drop)(files.to_vec());
            true
        });
        self.clear_targeted();
        accepted.unwrap_or(false)
    }

    /// How many files the zone under `point` accepts; `None` when it takes none.
    pub fn capacity(&self, point: Point) -> Option<usize> {
        self.zone_at(point)
            .filter(|node| {
                self.with_zone(node, |zone| zone.is_enabled)
                    .unwrap_or(false)
            })
            .map(|node| {
                self.with_zone(&node, |zone| zone.maximum_files)
                    .unwrap_or(usize::MAX)
            })
    }

    /// Runs `body` against the zone registered for `node`.
    ///
    /// The zone list is borrowed only for the call, so `body` never holds the
    /// registry lock while it runs caller code.
    fn with_zone<T>(&self, node: &Node, body: impl FnOnce(&Zone) -> T) -> Option<T> {
        let zones = self.zones.borrow();
        zones
            .iter()
            .find(|zone| Node::upgrade(&zone.node).is_some_and(|candidate| candidate == *node))
            .map(body)
    }

    fn accepts(files: &[DroppedFile], zone: &Zone) -> bool {
        !files.is_empty()
            && zone.is_enabled
            && files.len() <= zone.maximum_files
            && files
                .iter()
                .all(|file| file.matches_any(&zone.accepted_extensions))
            && zone.accepts.as_ref().is_none_or(|accepts| accepts(files))
    }

    fn zone_at(&self, point: Point) -> Option<Node> {
        let zones = self.zones.borrow();
        zones
            .iter()
            .filter_map(|zone| {
                let node = Node::upgrade(&zone.node)?;
                if !zone.is_enabled
                    || !node.is_layout_visible()
                    || !node.is_interaction_enabled()
                    || !contains(node.absolute_frame(), point)
                {
                    return None;
                }
                Some((node, depth_of(&Node::upgrade(&zone.node)?)))
            })
            .min_by(|(_, left), (_, right)| right.cmp(left))
            .map(|(node, _)| node)
    }

    fn clear_targeted(&self) {
        let previous = self.targeted.borrow_mut().take();
        if let Some(node) = previous.and_then(|node| Node::upgrade(&node))
            && let Some(on_targeted) = self
                .with_zone(&node, |zone| zone.on_targeted.clone())
                .flatten()
        {
            on_targeted(false);
        }
    }

    fn prune(&self) {
        self.zones
            .borrow_mut()
            .retain(|zone| Node::upgrade(&zone.node).is_some());
    }
}

fn contains(rect: Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.x <= rect.x + rect.width
        && point.y >= rect.y
        && point.y <= rect.y + rect.height
}

fn depth_of(node: &Node) -> usize {
    let mut depth = 0;
    let mut current = node.parent();
    while let Some(parent) = current {
        depth += 1;
        current = parent.parent();
    }
    depth
}

thread_local! {
    static CURRENT_REGISTRY: RefCell<Option<Rc<FileDropRegistry>>> = const { RefCell::new(None) };
}

/// Ambient access to the host-provided file drop registry.
pub struct FileDropRegistryHolder;

impl FileDropRegistryHolder {
    pub fn current() -> Option<Rc<FileDropRegistry>> {
        CURRENT_REGISTRY.with(|slot| slot.borrow().clone())
    }

    pub fn set_current(registry: Option<Rc<FileDropRegistry>>) {
        CURRENT_REGISTRY.with(|slot| *slot.borrow_mut() = registry);
    }
}

/// What the zone is showing right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileDropOutcome {
    #[default]
    Idle,
    Accepted,
    Rejected,
}

/// A drop area for files dragged in from outside the application.
///
/// `is_targeted` and `outcome` drive the chrome; `on_drop` runs only for a drop
/// the filter accepted. `is_enabled: false` removes the zone entirely, so a
/// disabled control never paints a border it cannot honour.
pub struct FileDropZone {
    pub accepted_extensions: HashSet<String>,
    pub maximum_files: usize,
    pub is_enabled: bool,
    /// The live drag is over this zone. Provided by the owner so the zone can be
    /// painted by a binding the host already drives.
    pub is_targeted: Binding<bool>,
    /// Whether the current drag carries a payload this zone accepts.
    pub outcome: Binding<FileDropOutcome>,
    on_drop: Rc<dyn Fn(Vec<DroppedFile>)>,
    on_targeted: Option<Rc<dyn Fn(bool)>>,
    accepts: Option<Rc<dyn Fn(&[DroppedFile]) -> bool>>,
    content: AnyView,
}

impl FileDropZone {
    pub fn new(on_drop: impl Fn(Vec<DroppedFile>) + 'static, content: impl View + 'static) -> Self {
        Self {
            accepted_extensions: HashSet::new(),
            maximum_files: usize::MAX,
            is_enabled: true,
            is_targeted: Binding::constant(false),
            outcome: Binding::constant(FileDropOutcome::Idle),
            on_drop: Rc::new(on_drop),
            on_targeted: None,
            accepts: None,
            content: AnyView::new(content),
        }
    }

    /// Restricts the drop to these extensions (no dot, case-insensitive).
    pub fn accepted_extensions(
        mut self,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.accepted_extensions = extensions
            .into_iter()
            .map(|extension| extension.into().to_ascii_lowercase())
            .collect();
        self
    }

    pub fn maximum_files(mut self, maximum_files: usize) -> Self {
        self.maximum_files = maximum_files.max(1);
        self
    }

    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn is_targeted(mut self, is_targeted: Binding<bool>) -> Self {
        self.is_targeted = is_targeted;
        self
    }

    pub fn outcome(mut self, outcome: Binding<FileDropOutcome>) -> Self {
        self.outcome = outcome;
        self
    }

    /// Called when the platform drag enters (`true`) or leaves (`false`) this
    /// zone. The owner writes its hover state from it.
    pub fn on_targeted(mut self, on_targeted: impl Fn(bool) + 'static) -> Self {
        self.on_targeted = Some(Rc::new(on_targeted));
        self
    }

    /// Narrows what the zone accepts beyond the extension list, e.g. a payload
    /// that is too large or a drag that arrived while the control was busy.
    pub fn accepts(mut self, accepts: impl Fn(&[DroppedFile]) -> bool + 'static) -> Self {
        self.accepts = Some(Rc::new(accepts));
        self
    }
}

impl View for FileDropZone {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(FileDropZoneHost {
            accepted_extensions: self.accepted_extensions.clone(),
            maximum_files: self.maximum_files,
            is_enabled: self.is_enabled,
            is_targeted: self.is_targeted.clone(),
            outcome: self.outcome.clone(),
            on_drop: self.on_drop.clone(),
            on_targeted: self.on_targeted.clone(),
            accepts: self.accepts.clone(),
            content: self.content.clone(),
        })
    }
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    is_targeted: bool,
    outcome: FileDropOutcome,
    is_enabled: bool,
}

struct FileDropZoneHost {
    accepted_extensions: HashSet<String>,
    maximum_files: usize,
    is_enabled: bool,
    is_targeted: Binding<bool>,
    outcome: Binding<FileDropOutcome>,
    on_drop: Rc<dyn Fn(Vec<DroppedFile>)>,
    on_targeted: Option<Rc<dyn Fn(bool)>>,
    accepts: Option<Rc<dyn Fn(&[DroppedFile]) -> bool>>,
    content: AnyView,
}

impl PrimitiveView for FileDropZoneHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        if let Some(registry) = FileDropRegistryHolder::current() {
            if self.is_enabled {
                registry.register(
                    node,
                    self.accepted_extensions.clone(),
                    self.maximum_files,
                    self.is_enabled,
                    self.on_drop.clone(),
                    self.on_targeted.clone(),
                    self.accepts.clone(),
                );
            } else {
                registry.unregister(node);
            }
        }
        if !self.is_enabled {
            node.update_overlay_draw((), |_, _| {});
            return;
        }
        let identity = PaintIdentity {
            is_targeted: self.is_targeted.get(),
            outcome: self.outcome.get(),
            is_enabled: self.is_enabled,
        };
        let outcome = identity.outcome;
        let is_targeted = identity.is_targeted;
        let draw_node = node.clone();
        node.update_overlay_draw(identity, move |list, origin| {
            if !is_targeted {
                return;
            }
            let theme = theme_for_node(&draw_node);
            let frame = draw_node.frame();
            let rect = Rect::new(origin.x, origin.y, frame.width, frame.height);
            let radius = theme.inputs.radius;
            let (fill, stroke) = match outcome {
                FileDropOutcome::Accepted => (
                    theme.colors.accent.multiplied_alpha(0.10),
                    theme.colors.accent.multiplied_alpha(0.85),
                ),
                FileDropOutcome::Rejected => (
                    theme.colors.error.multiplied_alpha(0.08),
                    theme.colors.error.multiplied_alpha(0.70),
                ),
                FileDropOutcome::Idle => (
                    theme.colors.accent.multiplied_alpha(0.04),
                    theme.inputs.border_color,
                ),
            };
            let _ = list.add_rounded_rect(rect, radius, fill);
            let _ = list.add_rounded_rect_stroke(
                rect,
                radius,
                theme.inputs.border_width.max(1.0),
                stroke,
            );
        });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

crate::impl_primitive_view!(FileDropZoneHost);

/// The visible body of an empty drop zone: an icon slot, a title, a hint and a
/// browse button for users who cannot drag.
///
/// The browse button is the accessible path to the same callback: it asks the
/// native open panel through `FilePicker`, so the zone is usable without a
/// pointer drag on every platform.
pub struct FileDropPrompt {
    pub title: String,
    pub hint: String,
    pub accepted_extensions: Vec<String>,
    pub maximum_files: usize,
    pub is_enabled: bool,
    on_browse: Rc<dyn Fn(Vec<DroppedFile>)>,
}

impl FileDropPrompt {
    /// `on_browse` receives what the user picked from the native panel.
    pub fn new(on_browse: impl Fn(Vec<DroppedFile>) + 'static) -> Self {
        Self {
            title: "Drop files here".into(),
            hint: String::new(),
            accepted_extensions: Vec::new(),
            maximum_files: usize::MAX,
            is_enabled: true,
            on_browse: Rc::new(on_browse),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    pub fn accepted_extensions(mut self, extensions: impl IntoIterator<Item = String>) -> Self {
        self.accepted_extensions = extensions.into_iter().collect();
        self
    }

    pub fn maximum_files(mut self, maximum_files: usize) -> Self {
        self.maximum_files = maximum_files.max(1);
        self
    }

    pub fn is_enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// The filter summary shown under the hint, e.g. `PNG, JPG · up to 4 files`.
    pub fn detail_text(&self) -> Option<String> {
        let types = if self.accepted_extensions.is_empty() {
            None
        } else {
            Some(
                self.accepted_extensions
                    .iter()
                    .map(|extension| extension.to_ascii_uppercase())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };
        let count = (self.maximum_files != usize::MAX)
            .then(|| format!("up to {} files", self.maximum_files));
        match (types, count) {
            (Some(types), Some(count)) => Some(format!("{types} · {count}")),
            (Some(types), None) => Some(types),
            (None, Some(count)) => Some(count),
            (None, None) => None,
        }
    }
}

impl View for FileDropPrompt {
    fn body(&self, _cx: &Scope) -> AnyView {
        let on_browse = self.on_browse.clone();
        let multiple = self.maximum_files > 1;
        let is_enabled = self.is_enabled;
        let filters = self.accepted_extensions.clone();
        let maximum_files = self.maximum_files;
        let browse = Button::with_options(
            Text::new("Browse…"),
            move || {
                if !is_enabled {
                    return;
                }
                let filters = filters.clone();
                let on_browse = on_browse.clone();
                cove_core::scene::FilePicker::request(
                    &[cove_core::scene::FilePickerFilter::new(
                        "Files",
                        filters.iter().map(|extension| format!(".{extension}")),
                    )],
                    multiple,
                    Box::new(move |paths| {
                        let files: Vec<DroppedFile> = paths
                            .into_iter()
                            .map(DroppedFile::new)
                            .take(maximum_files)
                            .collect();
                        if !files.is_empty() {
                            on_browse(files);
                        }
                    }),
                );
            },
            ButtonRole::Normal,
            is_enabled && cove_core::scene::FilePicker::is_installed(),
            false,
            false,
            None,
        );
        let mut rows = vec![
            AnyView::new(Text::new(self.title.clone())),
            AnyView::new(Text::new(self.hint.clone())),
        ];
        if let Some(detail) = self.detail_text() {
            rows.push(AnyView::new(Text::new(detail)));
        }
        rows.push(AnyView::new(browse));
        AnyView::new(
            crate::BoxView::new(
                FlexDirection::Column,
                Align::Center,
                cove_core::scene::Justify::Center,
                cove_core::scene::Wrap::NoWrap,
                8.0,
                AnyView::new(cove_composition::TupleView::new(rows)),
            )
            .accessibility(|semantics| {
                semantics.role = cove_core::scene::AccessibilityRole::Group;
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_the_lowercase_extension() {
        assert_eq!(DroppedFile::new("/tmp/Render.PNG").kind, "png");
        assert_eq!(DroppedFile::new(r"C:\art\a.b.MOV").kind, "mov");
        assert_eq!(DroppedFile::new("/tmp/README").kind, "");
    }

    #[test]
    fn name_is_the_last_component_for_both_separators() {
        assert_eq!(DroppedFile::new("/tmp/dir/file.png").name, "file.png");
        assert_eq!(DroppedFile::new(r"C:\dir\file.png").name, "file.png");
        assert_eq!(DroppedFile::new("file.png").name, "file.png");
    }

    #[test]
    fn empty_filter_accepts_every_file() {
        let file = DroppedFile::new("/tmp/a.bin");
        assert!(file.matches_any(&HashSet::new()));
        let png = HashSet::from(["png".to_owned()]);
        assert!(!file.matches_any(&png));
        assert!(DroppedFile::new("/tmp/a.PNG").matches_any(&png));
    }

    #[test]
    fn sizes_use_one_decimal_above_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn detail_text_joins_types_and_count() {
        let prompt =
            FileDropPrompt::new(|_| {}).accepted_extensions(vec!["png".into(), "jpg".into()]);
        assert_eq!(prompt.detail_text().as_deref(), Some("PNG, JPG"));
        let prompt = prompt.maximum_files(4);
        assert_eq!(
            prompt.detail_text().as_deref(),
            Some("PNG, JPG · up to 4 files")
        );
    }
}
