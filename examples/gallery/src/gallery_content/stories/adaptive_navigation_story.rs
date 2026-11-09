//! `ListDetail` and `AdaptiveTabs` under the live surface: resize the window
//! or switch device presets to see one pane, two panes, a fold-aligned split,
//! a bottom bar, a rail or top tabs.

use cove_composition::Scope;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Signal};
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::TabItem;
use cove_ui_compose::primitives::navigation::{
    AdaptiveTabs, ListDetail, ListDetailContext, ListDetailState,
};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::{AnyView, BoxView, LayoutDebugExt, TupleView, View};

use crate::gallery_content::stories::StorySection;

const ITEMS: [&str; 8] = [
    "Inbox", "Drafts", "Sent", "Archive", "Projects", "Receipts", "Travel", "Family",
];

pub struct AdaptiveNavigationStory;

impl AdaptiveNavigationStory {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AdaptiveNavigationStory {
    fn default() -> Self {
        Self::new()
    }
}

fn column(spacing: f32, children: Vec<AnyView>) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(children)),
    ))
}

fn list(selection: Binding<Option<usize>>) -> AnyView {
    let rows = ITEMS
        .iter()
        .enumerate()
        .map(|(index, title)| {
            let selection = selection.clone();
            AnyView::new(
                AnyView::new(Button::new(Text::new(*title), move || {
                    selection.set(Some(index))
                }))
                .debug_name(&format!("list-detail-row-{index}")),
            )
        })
        .collect();
    AnyView::new(column(4.0, rows).padding(12.0))
}

fn detail(index: usize, context: ListDetailContext) -> AnyView {
    let mut children = Vec::new();
    if context.is_single_pane {
        let back = context.back.clone();
        children.push(AnyView::new(
            AnyView::new(Button::new(Text::new("Back"), move || back()))
                .debug_name("list-detail-back"),
        ));
    }
    children.push(AnyView::new(
        Text::new(ITEMS[index % ITEMS.len()]).debug_name("list-detail-title"),
    ));
    AnyView::new(column(12.0, children).padding(16.0))
}

impl View for AdaptiveNavigationStory {
    fn body(&self, cx: &Scope) -> AnyView {
        let state: Signal<ListDetailState<usize>> = cx.state(ListDetailState::default);
        let tab: Signal<u8> = cx.state(|| 0);
        let list_detail = AnyView::new(
            AnyView::new(
                ListDetail::new(
                    Binding::new(move || state.get(), move |value| state.set(value)),
                    list,
                    detail,
                )
                .placeholder(Text::new("—")),
            )
            .frame_height(420.0)
            .debug_name("adaptive-list-detail"),
        );
        let tabs = AdaptiveTabs::try_new(
            Binding::new(move || tab.get(), move |value| tab.set(value)),
            vec![
                TabItem::new(0, "Home", Text::new("Home").padding(16.0)),
                TabItem::new(1, "Search", Text::new("Search").padding(16.0)),
                TabItem::new(2, "Library", Text::new("Library").padding(16.0)),
            ],
        )
        .map(AnyView::new)
        .unwrap_or_else(|_| AnyView::new(cove_ui_compose::EmptyView));
        column(
            24.0,
            vec![
                AnyView::new(StorySection::new("ListDetail", "", list_detail)),
                AnyView::new(StorySection::new(
                    "AdaptiveTabs",
                    "",
                    AnyView::new(tabs.frame_height(260.0).debug_name("adaptive-tabs")),
                )),
            ],
        )
    }
}
