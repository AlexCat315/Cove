//! Translation of Primitives/Data/Lists/SectionedList.swift.
//!
//! Vertically virtualized list with flat section headers and footers. Selection
//! and keyboard navigation only visit item rows; headers are non-focusable.
//! With sticky headers, the active section title stays pinned to the top of the
//! viewport and is pushed out by the next section header.
//!
//! Behaviour checklist:
//! - only the rows inside the viewport plus overscan create nodes
//! - a header/footer entry exists only when its authored text is non-empty
//! - Down/Up/Home/End step between item rows, skipping headers and footers
//! - Return activates the cursor row, or the selected row when it has no cursor
//! - the pinned header appears only once its inline row has scrolled past the
//!   top, and the next header pushes it up

use std::rc::Rc;

use cove_composition::{LayoutDebugExt, Scancode};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Wrap,
};
use cove_core::{Binding, Environment, InputCapabilities, Point, Size};
use cove_platform::KeyEvent;
use crate::adaptive::{ContainerSizeReader, SurfaceReader};
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{
    AbsolutePositionModifier, EdgeInsets, FlexExt, FrameDimension, LayoutFrameExt, PaddingExt,
};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::modifiers::interaction::HitTestingExt;
use crate::primitives::layout::virtualization::FixedVirtualStack;
use crate::primitives::overlays::selection_key_host::SelectionKeyHost;
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::{AnyView, BoxView, TupleView, View, ViewExt};
use super::list::ListRowHost;

/// One flat section in a [`SectionedList`]. Headers and footers are optional;
/// headers can be pinned while their section scrolls when `sticky_headers` is
/// set on the list.
#[derive(Clone, Debug, PartialEq)]
pub struct ListSection<SectionId: Clone, Item: Clone> {
    pub id: SectionId,
    pub title: Option<String>,
    pub footer: Option<String>,
    pub items: Vec<Item>,
}

impl<SectionId: Clone, Item: Clone> ListSection<SectionId, Item> {
    pub fn new(id: SectionId, items: Vec<Item>) -> Self {
        Self { id, title: None, footer: None, items }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<String>) -> Self {
        self.footer = Some(footer.into());
        self
    }
}

/// Identity of one entry in the flattened section stream.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SectionedEntryId<SectionId, ItemId> {
    Header(SectionId),
    Footer(SectionId),
    Item(ItemId),
}

/// One entry: a header, a footer or an item row.
#[derive(Clone)]
enum Entry<SectionId, Item> {
    Header(SectionId, String),
    Footer(SectionId, String),
    Item(Item),
}

/// A pinned section title and its vertical offset from the viewport top.
#[derive(Clone, Debug, PartialEq)]
pub struct StickyHeader {
    pub title: String,
    pub y: f32,
}

/// The flattened entry stream: per section, a header then its items then its
/// footer, skipping empty authored text.
fn flatten<SectionId: Clone, Item: Clone>(
    sections: &[ListSection<SectionId, Item>],
) -> Vec<Entry<SectionId, Item>> {
    let mut result = Vec::new();
    for section in sections {
        if let Some(title) = section.title.as_deref().filter(|value| !value.is_empty()) {
            result.push(Entry::Header(section.id.clone(), title.to_string()));
        }
        for item in &section.items {
            result.push(Entry::Item(item.clone()));
        }
        if let Some(footer) = section.footer.as_deref().filter(|value| !value.is_empty()) {
            result.push(Entry::Footer(section.id.clone(), footer.to_string()));
        }
    }
    result
}

/// The header that should stay pinned at `offset`, or `None` when the active
/// inline header has not scrolled past the top yet.
fn resolve_sticky_header<SectionId, Item>(
    entries: &[Entry<SectionId, Item>],
    offset: f32,
    row_height: f32,
    spacing: f32,
    sticky_headers: bool,
) -> Option<StickyHeader> {
    if !sticky_headers || entries.is_empty() {
        return None;
    }
    let stride = (row_height + spacing).max(1.0);
    let mut active: Option<(usize, String)> = None;
    let mut next_top: Option<f32> = None;
    for (index, entry) in entries.iter().enumerate() {
        let Entry::Header(_, title) = entry else { continue };
        let top = index as f32 * stride;
        if top <= offset + 0.5 {
            active = Some((index, title.clone()));
        } else {
            next_top = Some(top);
            break;
        }
    }
    let (index, title) = active?;
    if offset <= index as f32 * stride + 0.5 {
        return None;
    }
    let y = match next_top {
        Some(next_top) => (next_top - offset - row_height).min(0.0),
        None => 0.0,
    };
    Some(StickyHeader { title, y })
}

/// Vertically virtualized list with flat section headers and footers.
pub struct SectionedList<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    pub sections: Vec<ListSection<SectionId, Item>>,
    pub selection: Binding<Option<ItemId>>,
    /// Authored logical row height; `None` uses the surface-dependent default.
    pub row_height: Option<f32>,
    pub row_spacing: f32,
    pub sticky_headers: bool,
    pub sticky_header_background: SemanticColorRef,
    pub on_activate: Option<Rc<dyn Fn(&Item)>>,
    pub row_content: Rc<dyn Fn(&Item, bool) -> AnyView>,
    id_of: Rc<dyn Fn(&Item) -> ItemId>,
    cursor: cove_core::State<Option<ItemId>>,
    scroll_offset: cove_core::State<f32>,
}

impl<SectionId, Item, ItemId> SectionedList<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    pub fn new(
        sections: Vec<ListSection<SectionId, Item>>,
        id_of: impl Fn(&Item) -> ItemId + 'static,
        row_content: impl Fn(&Item, bool) -> AnyView + 'static,
    ) -> Self {
        Self {
            sections,
            selection: Binding::constant(None),
            row_height: None,
            row_spacing: 0.0,
            sticky_headers: false,
            sticky_header_background: SemanticColorRef::surface_raised(),
            on_activate: None,
            row_content: Rc::new(row_content),
            id_of: Rc::new(id_of),
            cursor: cove_core::State::new(None),
            scroll_offset: cove_core::State::new(0.0),
        }
    }

    pub fn selection(mut self, selection: Binding<Option<ItemId>>) -> Self {
        self.selection = selection;
        self
    }

    pub fn row_height(mut self, row_height: Option<f32>) -> Self {
        self.row_height =
            row_height.map(|value| if value.is_finite() { value.max(1.0) } else { 30.0 });
        self
    }

    pub fn row_spacing(mut self, row_spacing: f32) -> Self {
        self.row_spacing = row_spacing.max(0.0);
        self
    }

    pub fn sticky_headers(mut self, sticky_headers: bool) -> Self {
        self.sticky_headers = sticky_headers;
        self
    }

    pub fn sticky_header_background(mut self, background: SemanticColorRef) -> Self {
        self.sticky_header_background = background;
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(&Item) + 'static) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

}

impl<SectionId, Item, ItemId> View for SectionedList<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    fn body(&self) -> AnyView {
        let entries = flatten(&self.sections);
        let ids: Vec<SectionedEntryId<SectionId, ItemId>> = entries
            .iter()
            .map(|entry| match entry {
                Entry::Header(id, _) => SectionedEntryId::Header(id.clone()),
                Entry::Footer(id, _) => SectionedEntryId::Footer(id.clone()),
                Entry::Item(item) => SectionedEntryId::Item((self.id_of)(item)),
            })
            .collect();

        let surface = SectionedListSurface {
            entries: Rc::new(entries),
            ids,
            cursor: self.cursor.clone(),
            selection: self.selection.clone(),
            offset: self.scroll_offset.clone(),
            sticky_headers: self.sticky_headers,
            sticky_background: self.sticky_header_background.clone(),
            row_height: self.row_height,
            row_spacing: self.row_spacing,
            on_activate: self.on_activate.clone(),
            row_content: self.row_content.clone(),
            id_of: self.id_of.clone(),
        };
        let keys = surface.clone_for_keys();
        AnyView::new(SelectionKeyHost::new(
            ContainerSizeReader::new({
                let surface = surface.clone();
                move |size: Size| {
                    let surface = surface.clone();
                    SurfaceReader::new({
                        let surface = surface;
                        move |environment: &Environment| {
                            surface.render(size, environment)
                        }
                    })
                }
            }),
            true,
            move |event: &KeyEvent| keys.handle_key(event),
        ))
    }
}

/// The session values the keyboard boundary and the rendered surface share.
#[derive(Clone)]
struct SectionedListKeys<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    entries: Rc<Vec<Entry<SectionId, Item>>>,
    id_of: Rc<dyn Fn(&Item) -> ItemId>,
    cursor: cove_core::State<Option<ItemId>>,
    selection: Binding<Option<ItemId>>,
    on_activate: Option<Rc<dyn Fn(&Item)>>,
}

impl<SectionId, Item, ItemId> SectionedListKeys<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    fn item_positions(&self) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| matches!(entry, Entry::Item(_)).then_some(index))
            .collect()
    }

    fn id_at(&self, position: usize) -> Option<ItemId> {
        match self.entries.get(position) {
            Some(Entry::Item(item)) => Some((self.id_of)(item)),
            _ => None,
        }
    }

    fn handle_key(&self, event: &KeyEvent) -> bool {
        let items = self.item_positions();
        if items.is_empty() {
            return false;
        }
        let position_of = |id: &ItemId| {
            items
                .iter()
                .copied()
                .find(|position| self.id_at(*position).as_ref() == Some(id))
        };
        let current = self
            .cursor
            .get()
            .as_ref()
            .and_then(position_of)
            .or_else(|| self.selection.get().as_ref().and_then(position_of));

        if event.scancode == Scancode::RETURN || event.scancode == Scancode::KEYPAD_ENTER {
            let Some(position) = current else { return false };
            let Some(Entry::Item(item)) = self.entries.get(position) else { return false };
            let id = (self.id_of)(item);
            self.cursor.set(Some(id.clone()));
            self.selection.set(Some(id));
            if let Some(handler) = &self.on_activate {
                handler(item);
            }
            return true;
        }

        let target = match event.scancode {
            Scancode::ARROW_DOWN => {
                let next = current.map(|value| value + 1).unwrap_or(0);
                items[next.min(items.len() - 1)]
            }
            Scancode::ARROW_UP => {
                let next = current
                    .and_then(|value| value.checked_sub(1))
                    .unwrap_or(items.len() - 1);
                items[next.min(items.len() - 1)]
            }
            Scancode::HOME => items[0],
            Scancode::END => items[items.len() - 1],
            _ => return false,
        };

        if let Some(id) = self.id_at(target) {
            self.cursor.set(Some(id.clone()));
            self.selection.set(Some(id));
        }
        true
    }
}

/// Entry rows in a fixed virtual stack plus the pinned header band.
struct SectionedListSurface<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    entries: Rc<Vec<Entry<SectionId, Item>>>,
    ids: Vec<SectionedEntryId<SectionId, ItemId>>,
    cursor: cove_core::State<Option<ItemId>>,
    selection: Binding<Option<ItemId>>,
    offset: cove_core::State<f32>,
    sticky_headers: bool,
    sticky_background: SemanticColorRef,
    row_height: Option<f32>,
    row_spacing: f32,
    on_activate: Option<Rc<dyn Fn(&Item)>>,
    row_content: Rc<dyn Fn(&Item, bool) -> AnyView>,
    id_of: Rc<dyn Fn(&Item) -> ItemId>,
}

impl<SectionId, Item, ItemId> Clone for SectionedListSurface<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            ids: self.ids.clone(),
            cursor: self.cursor.clone(),
            selection: self.selection.clone(),
            offset: self.offset.clone(),
            sticky_headers: self.sticky_headers,
            sticky_background: self.sticky_background.clone(),
            row_height: self.row_height,
            row_spacing: self.row_spacing,
            on_activate: self.on_activate.clone(),
            row_content: self.row_content.clone(),
            id_of: self.id_of.clone(),
        }
    }
}

impl<SectionId, Item, ItemId> SectionedListSurface<SectionId, Item, ItemId>
where
    SectionId: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    fn clone_for_keys(&self) -> SectionedListKeys<SectionId, Item, ItemId> {
        SectionedListKeys {
            entries: self.entries.clone(),
            id_of: self.id_of.clone(),
            cursor: self.cursor.clone(),
            selection: self.selection.clone(),
            on_activate: self.on_activate.clone(),
        }
    }

    fn cursor_index(&self) -> Option<usize> {
        let id = self.cursor.get()?;
        self.ids.iter().position(|entry| *entry == SectionedEntryId::Item(id.clone()))
    }

    fn render(&self, size: Size, environment: &Environment) -> AnyView {
        let touch = environment.input.contains(InputCapabilities::TOUCH);
        let authored = self.row_height.unwrap_or(if touch { 56.0 } else { 30.0 });
        let height = authored.max(if touch { environment.minimum_control_extent() } else { 0.0 });
        let spacing = self.row_spacing;
        let sticky = resolve_sticky_header(
            &self.entries,
            self.offset.get(),
            height,
            spacing,
            self.sticky_headers,
        );

        let ids = self.ids.clone();
        let rows = self.entries.clone();
        let selection = self.selection.clone();
        let cursor = self.cursor.clone();
        let content = self.row_content.clone();
        let id_of = self.id_of.clone();
        let on_activate = self.on_activate.clone();

        let mut stack = FixedVirtualStack::new(ids.clone(), height, move |id: &SectionedEntryId<SectionId, ItemId>| {
            let Some(position) = ids.iter().position(|entry| entry == id) else {
                return AnyView::new(crate::EmptyView);
            };
            entry_view(
                &rows,
                position,
                height,
                touch,
                &selection,
                &cursor,
                &id_of,
                &content,
                &on_activate,
            )
        })
        .spacing(spacing);
        if let Some(index) = self.cursor_index() {
            stack = stack.scroll_to_index(index);
        }
        let offset = self.offset.clone();
        stack = stack.on_scroll_offset_change(move |point: Point| {
            if (point.y - offset.get()).abs() > 0.25 {
                offset.set(point.y);
            }
        });

        let list = AnyView::new(
            AnyView::new(stack)
                .frame(None, None, Some(0.0), Some(0.0), None, None)
                .flex_with(1.0, 1.0, Some(0.0)),
        );

        let mut layers = vec![list];
        if let Some(header) = sticky {
            layers.push(AnyView::new(
                AnyView::new(sticky_header_view(
                    &header,
                    height,
                    touch,
                    size.width,
                    self.sticky_background.clone(),
                ))
                .modifier(AbsolutePositionModifier::new(
                    Some(0.0),
                    Some(header.y),
                    None,
                    None,
                )),
            ));
        }

        AnyView::new(
            AnyView::new(crate::primitives::layout::ZStack::new(
                crate::Alignment::TopLeading,
                AnyView::new(TupleView::new(layers)),
            ))
            .clipped()
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn entry_view<SectionId, Item, ItemId>(
    rows: &[Entry<SectionId, Item>],
    position: usize,
    height: f32,
    touch: bool,
    selection: &Binding<Option<ItemId>>,
    cursor: &cove_core::State<Option<ItemId>>,
    id_of: &Rc<dyn Fn(&Item) -> ItemId>,
    content: &Rc<dyn Fn(&Item, bool) -> AnyView>,
    on_activate: &Option<Rc<dyn Fn(&Item)>>,
) -> AnyView
where
    Item: Clone + 'static,
    ItemId: Ord + Clone + Eq + std::hash::Hash + Send + Sync + 'static,
{
    match rows.get(position) {
        Some(Entry::Header(_id, title)) => {
            let title = title.clone();
            AnyView::new(
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::FlexStart,
                        if touch { Justify::FlexEnd } else { Justify::Center },
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(
                            Text::new(title.clone())
                                .line_limit(Some(1))
                                .semantic_font(SemanticFontRef::caption())
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                    )
                    .padding_insets(if touch {
                        EdgeInsets::new(0.0, 16.0, 6.0, 16.0)
                    } else {
                        EdgeInsets::new(0.0, 10.0, 0.0, 10.0)
                    })
                    .frame_height(height),
                )
                .accessibility({
                    let title = title.clone();
                    move |semantics: &mut AccessibilitySemantics| {
                        semantics.role = AccessibilityRole::StaticText;
                        semantics.label = title.clone();
                        semantics.combines_children = true;
                    }
                }),
            )
        }
        Some(Entry::Footer(_id, text)) => {
            let text = text.clone();
            AnyView::new(
                AnyView::new(
                    Text::new(text.clone())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .padding_edges(if touch { 16.0 } else { 10.0 }, 0.0)
                        .frame_height(height),
                )
                .accessibility({
                    let text = text.clone();
                    move |semantics: &mut AccessibilitySemantics| {
                        semantics.role = AccessibilityRole::StaticText;
                        semantics.label = text.clone();
                    }
                }),
            )
        }
        Some(Entry::Item(item)) => {
            let id = (id_of)(item);
            let selected = selection.get().as_ref() == Some(&id);
            let view = content(item, selected);
            let activate_item = item.clone();
            let handler = on_activate.clone();
            let cursor = cursor.clone();
            let selection = selection.clone();
            let on_activate: Rc<dyn Fn()> = Rc::new({
                let id = id.clone();
                move || {
                    cursor.set(Some(id.clone()));
                    selection.set(Some(id.clone()));
                    if let Some(handler) = &handler {
                        handler(&activate_item);
                    }
                }
            });
            AnyView::new(ListRowHost {
                id: 0,
                is_selected: selected,
                row_height: height,
                on_activate,
                on_tap: None,
                content: view,
            })
        }
        None => AnyView::new(crate::EmptyView),
    }
}

fn sticky_header_view(
    header: &StickyHeader,
    height: f32,
    touch: bool,
    width: f32,
    background: SemanticColorRef,
) -> AnyView {
    let title = header.title.clone();
    AnyView::new(
        AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                if touch { Align::FlexEnd } else { Align::Center },
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    Text::new(title.clone())
                        .line_limit(Some(1))
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .padding_insets(if touch {
                            EdgeInsets::new(0.0, 16.0, 6.0, 16.0)
                        } else {
                            EdgeInsets::new(0.0, 10.0, 0.0, 10.0)
                        }),
                ),
            )
            .frame(
                Some(FrameDimension::Points(width.max(1.0))),
                Some(FrameDimension::Points(height)),
                None,
                None,
                None,
                None,
            )
            .semantic_background(background),
        )
        .accessibility_hidden(true)
        .allows_hit_testing(false)
        .debug_name("sectioned-list-sticky-header"),
    )
}
