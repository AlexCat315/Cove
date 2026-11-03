//! The gallery shell: navigation, the page header, the control toolbar, the story
//! outlet and the footer.
//!
//! Translation of `Examples/Sources/GalleryContent/GalleryView.swift`. Story
//! routing, the catalog and the navigation icons live in the sibling modules;
//! this file is the frame around them.
//!
//! Structural difference from Swift: the shell composes [`Sidebar`] inside
//! [`ViewThatFits`] rather than `AdaptiveSidebar`, because the Rust
//! `AdaptiveSidebar` exposes no `header`/`footer` seam. Both branches keep the
//! Swift behaviour — a docked sidebar above the compact breakpoint, a rail with
//! icon-only affordances below it — and the header/footer content is authored
//! once and placed in each branch, which is what the Swift `wide`/`compact`
//! builder pairs did.
use cove_composition::Scope;
use cove_composition::{AnyView, LayoutDebugExt, View, ViewExt};
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};
use cove_core::{Binding, State};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::Checkbox;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::controls::{ButtonIcon, ButtonIconSource};
use cove_ui_compose::primitives::layout::ScrollView;
use cove_ui_compose::primitives::layout::insets::SafeArea;
use cove_ui_compose::primitives::layout::responsive::view_that_fits::ViewThatFits;
use cove_ui_compose::primitives::layout::sidebar::{
    AdaptiveSidebar, AdaptiveSidebarOptions, SidebarItem, SidebarItemStyle, SidebarSection,
};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::rich::text_selection::TextSelectionViewExt;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::buttons::{
    ButtonStyleViewExt, GhostButtonStyle, SecondaryButtonStyle,
};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticFontRef, SemanticStyleViewExt};
use cove_ui_compose::theme::{TextScalerReader, TextScalerViewExt, theme_for_node};

use crate::gallery_content::catalog::{
    CowSize, GalleryCatalog, GalleryEntry, GalleryOptions, GalleryPage,
};
use crate::gallery_content::routing::StoryDispatch;

/// Erase any view so modifier chains can be stored or returned as `AnyView`.
fn erased(view: impl View + 'static) -> AnyView {
    AnyView::new(view)
}

/// Sidebar breakpoint between the docked and collapsed presentations.
const NAVIGATION_RAIL_THRESHOLD: f32 = 120.0;
/// Supplied text scale choices.
const TEXT_SCALE_CHOICES: [(f64, &str); 4] =
    [(1.0, "100%"), (1.25, "125%"), (1.5, "150%"), (2.0, "200%")];

/// The gallery shell.
///
/// Navigation state belongs to the shell; each story owns and resets its own
/// example state through `reset_generation`.
pub struct GalleryView {
    pub page: State<GalleryPage>,
    pub search: State<TextBuffer>,
    pub appearance: State<Appearance>,
    pub options: State<GalleryOptions>,
    pub reset_generation: State<u32>,
    pub text_scale: State<Option<f64>>,
    pub navigation_open: State<bool>,
    pub navigation_collapsed: State<bool>,
    pub navigation_width: State<f32>,
}

impl GalleryView {
    pub fn new(initial_page: GalleryPage, appearance: Appearance, text_scale: f64) -> Self {
        let text_scale = if text_scale.is_finite() {
            text_scale.clamp(1.0, 2.0)
        } else {
            1.0
        };
        Self {
            page: State::new(initial_page),
            search: State::new(TextBuffer::default()),
            appearance: State::new(appearance),
            options: State::new(GalleryOptions::default()),
            reset_generation: State::new(0),
            text_scale: State::new(Some(text_scale)),
            navigation_open: State::new(false),
            navigation_collapsed: State::new(false),
            navigation_width: State::new(240.0),
        }
    }

    pub fn current_page(&self) -> GalleryPage {
        self.page.get()
    }

    pub fn is_dark(&self) -> bool {
        self.appearance.get() == Appearance::Dark
    }

    pub fn select_page(&self, page: GalleryPage) {
        self.page.set(page);
    }

    pub fn toggle_theme(&self) {
        self.appearance.set(match self.appearance.get() {
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
        });
    }

    /// Matches the catalog filtered by the search field, grouped into sections in
    /// catalog order. A category with no match is dropped.
    pub fn navigation_sections(&self) -> Vec<SidebarSection<GalleryPage>> {
        let query = self.search.get().string_value();
        let matches = GalleryCatalog::search(&query);
        GalleryCatalog::categories()
            .into_iter()
            .filter_map(|category| {
                let items: Vec<SidebarItem<GalleryPage>> = matches
                    .iter()
                    .filter(|entry| entry.category == category)
                    .map(|entry| {
                        SidebarItem::new(entry.page, entry.title).icon(entry.page.navigation_icon())
                    })
                    .collect();
                (!items.is_empty()).then(|| SidebarSection::new(category, items).title(category))
            })
            .collect()
    }

    fn search_binding(&self) -> Binding<TextBuffer> {
        self.search.binding()
    }

    fn selection_binding(&self) -> Binding<Option<GalleryPage>> {
        let page = self.page.clone();
        Binding::new(
            {
                let page = page.clone();
                move || Some(page.get())
            },
            {
                let page = page.clone();
                move |value: Option<GalleryPage>| {
                    if let Some(selected) = value {
                        page.set(selected);
                    }
                }
            },
        )
    }
}

impl View for GalleryView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let content = GalleryPagePane {
            page: self.page.clone(),
            options: self.options.clone(),
            reset_generation: self.reset_generation.clone(),
            text_scale: self.text_scale.clone(),
        };
        let mut options = AdaptiveSidebarOptions::default();
        options.allows_edge_swipe = true;
        options.layout.shows_rail_labels = false;
        let navigation = AdaptiveSidebar::try_new(
            "CoveUI navigation",
            self.selection_binding(),
            self.navigation_sections(),
        )
        .expect("unique sidebar IDs")
        .presentation(self.navigation_open.binding())
        .collapse(self.navigation_collapsed.binding())
        .width(self.navigation_width.binding())
        .options(options)
        .brand(self.navigation_brand())
        .header(ViewThatFits::threshold(
            NAVIGATION_RAIL_THRESHOLD,
            self.navigation_header(),
            self.navigation_header_compact(),
        ))
        .footer(ViewThatFits::threshold(
            NAVIGATION_RAIL_THRESHOLD,
            self.navigation_footer(),
            self.navigation_footer_compact(),
        ))
        .content(content.text_selection(true));
        let shell = AnyView::new(navigation)
            .flex_with(1.0, 1.0, Some(0.0))
            .frame(None, None, None, Some(0.0), None, None)
            .debug_name("gallery-shell");
        let root = AnyView::new(SafeArea::all(LayerRoot::new(shell)))
            .flex_with(1.0, 1.0, Some(0.0))
            .modifier(GalleryShellFrame)
            // The app background paints behind the system bars; the window is
            // white, the sidebar carries the surface fill, and the cards carry
            // their own edge.
            .semantic_background(SemanticColorRef::background());
        erased(AnyView::new(root).appearance(self.appearance.get(), ThemeTransition::circular()))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        use cove_composition::DynamicField;
        vec![
            DynamicField::state("page", &self.page),
            DynamicField::state("search", &self.search),
            DynamicField::state("appearance", &self.appearance),
            DynamicField::state("options", &self.options),
            DynamicField::state("reset_generation", &self.reset_generation),
            DynamicField::state("text_scale", &self.text_scale),
            DynamicField::state("navigation_open", &self.navigation_open),
            DynamicField::state("navigation_collapsed", &self.navigation_collapsed),
            DynamicField::state("navigation_width", &self.navigation_width),
        ]
    }
}

impl GalleryView {
    fn navigation_brand(&self) -> AnyView {
        let brand = AnyView::new(
            cove_ui_compose::BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    Text::new("G")
                        .semantic_font(SemanticFontRef::body_strong())
                        .semantic_foreground(SemanticColorRef::on_accent()),
                ),
            )
            .frame_size(32.0, 32.0)
            .corner_radius(10.0)
            .semantic_background(SemanticColorRef::accent()),
        );
        let titles = AnyView::new(cove_ui_compose::BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            2.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(Text::new("CoveUI").semantic_font(SemanticFontRef::body_strong())),
                AnyView::new(
                    Text::new("Component showcase").semantic_font(SemanticFontRef::caption()),
                ),
            ])),
        ));
        AnyView::new(
            cove_ui_compose::BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                10.0,
                AnyView::new(cove_composition::TupleView::new(vec![brand, titles])),
            )
            .padding_edges(10.0, 14.0),
        )
    }

    fn navigation_header(&self) -> AnyView {
        let search = erased(
            AnyView::new(
                cove_ui_compose::primitives::controls::fields::SearchField::new(
                    self.search_binding(),
                )
                .placeholder("Search components…"),
            )
            .debug_name("gallery-search"),
        );

        let mut children = vec![search];
        if self.navigation_sections().is_empty() {
            children.push(erased(
                AnyView::new(
                    Text::new("No matching components")
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted()),
                )
                .padding_edges(10.0, 12.0),
            ));
        }
        AnyView::new(cove_ui_compose::BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::TupleView::new(children)),
        ))
    }

    fn navigation_header_compact(&self) -> AnyView {
        erased(
            AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    Text::new("G")
                        .semantic_font(SemanticFontRef::headline())
                        .semantic_foreground(SemanticColorRef::accent()),
                ),
            ))
            .padding_edges(0.0, 10.0),
        )
    }

    fn navigation_footer(&self) -> AnyView {
        let title = match self.appearance.get() {
            Appearance::Light => "Switch to dark theme",
            Appearance::Dark => "Switch to light theme",
        };
        let view = self.clone_handle();
        let icon = match self.appearance.get() {
            Appearance::Light => icons::device::moon(),
            Appearance::Dark => icons::device::sun(),
        };
        let label = cove_ui_compose::BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(ButtonIcon::new(
                    ButtonIconSource::resource(icon),
                    Some(16.0),
                    None,
                )),
                AnyView::new(
                    Text::new(title)
                        .truncated()
                        .semantic_font(SemanticFontRef::label()),
                ),
            ])),
        )
        .padding_edges(10.0, 0.0);
        erased(
            AnyView::new(Button::new(label, move || view.toggle_theme()))
                .button_style(SidebarItemStyle {
                    minimum_height: 38.0,
                })
                .debug_name("gallery-theme"),
        )
    }

    fn navigation_footer_compact(&self) -> AnyView {
        let icon = match self.appearance.get() {
            Appearance::Light => icons::device::moon(),
            Appearance::Dark => icons::device::sun(),
        };
        let tooltip = match self.appearance.get() {
            Appearance::Light => "Switch to dark theme",
            Appearance::Dark => "Switch to light theme",
        };
        let view = self.clone_handle();
        erased(
            AnyView::new(
                Button::icon(
                    cove_ui_compose::primitives::controls::ButtonIconSource::resource(icon),
                    16.0,
                    move || view.toggle_theme(),
                )
                .tooltip(tooltip),
            )
            .button_style(GhostButtonStyle)
            .modifier(GalleryCompactFooterFrame)
            .debug_name("gallery-theme-rail"),
        )
    }

    /// A lightweight handle for closures that only need the theme toggle. State
    /// values are reference-counted, so this shares the same storage.
    fn clone_handle(&self) -> GalleryThemeToggle {
        GalleryThemeToggle {
            appearance: self.appearance.clone(),
        }
    }
}

/// Theme toggle shared by the docked and rail footers.
struct GalleryThemeToggle {
    appearance: State<Appearance>,
}

impl GalleryThemeToggle {
    fn toggle_theme(&self) {
        self.appearance.set(match self.appearance.get() {
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
        });
    }
}

struct GalleryCompactFooterFrame;

impl cove_composition::ViewModifier for GalleryCompactFooterFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_min_width(Some(44.0));
        layout.set_min_height(Some(44.0));
    }
}

struct GalleryShellFrame;

impl cove_composition::ViewModifier for GalleryShellFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_width_percent(100.0);
        layout.set_height_percent(100.0);
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
    }
}

/// Page selection is read inside the outlet, away from the adaptive shell.
struct GalleryPagePane {
    page: State<GalleryPage>,
    options: State<GalleryOptions>,
    reset_generation: State<u32>,
    text_scale: State<Option<f64>>,
}

impl View for GalleryPagePane {
    fn body(&self, _cx: &Scope) -> AnyView {
        let page = self.page.get();
        let options = self.options.get();
        let generation = self.reset_generation.get();
        let scale = self.text_scale.get().unwrap_or(1.0) as f32;

        let header = GalleryPageHeader { page };
        let toolbar = GalleryToolbar {
            options: self.options.clone(),
            text_scale: self.text_scale.clone(),
            reset_generation: self.reset_generation.clone(),
        };
        let dispatch = StoryDispatch::new(page);
        let story = match dispatch.view(options) {
            Some(story) => AnyView::new(TextScalerReader::new({
                let story = story.clone();
                move |inherited| {
                    erased(
                        AnyView::new(story.clone())
                            .modifier(GalleryStoryFrame)
                            .text_scaler(inherited.multiplied(scale))
                            .debug_name(&format!("{}-{generation}", page.raw_value())),
                    )
                }
            })),
            None => AnyView::new(GalleryPendingStory {
                message: dispatch.pending_message(),
            }),
        };
        let outlet = erased(
            AnyView::new(ScrollView::vertical(story))
                .flex_with(1.0, 1.0, Some(0.0))
                .debug_name("gallery-content"),
        );
        let footer = erased(
            AnyView::new(
                Text::new(format!(
                    "{} stories   ·   CoveUI component showcase",
                    GalleryCatalog::entries().len()
                ))
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
            )
            .padding_edges(24.0, 8.0),
        );
        let body = erased(
            AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(cove_composition::TupleView::new(vec![
                    header.view(),
                    toolbar.view(),
                    outlet,
                    footer,
                ])),
            ))
            .flex_with(1.0, 1.0, Some(0.0))
            .frame_min_width(0.0)
            .modifier(GalleryPageFrame),
        );
        erased(
            AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                body,
            ))
            .flex_with(1.0, 1.0, Some(0.0))
            .frame_min_width(0.0),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; page, options, reset_generation, text_scale)
    }
}

struct GalleryPageFrame;

impl cove_composition::ViewModifier for GalleryPageFrame {
    fn apply_node(&self, node: &Node) {
        let colors = theme_for_node(node).colors;
        node.set_background_color(Some(colors.background));
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
    }
}

struct GalleryStoryFrame;

impl cove_composition::ViewModifier for GalleryStoryFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_flex_shrink(0.0);
        layout.set_min_width(Some(0.0));
        layout.set_padding(24.0, cove_core::scene::Edge::Left);
        layout.set_padding(24.0, cove_core::scene::Edge::Right);
        layout.set_padding(24.0, cove_core::scene::Edge::Top);
        layout.set_padding(24.0, cove_core::scene::Edge::Bottom);
    }
}

/// A page whose story still needs Compose widgets. The message names the exact
/// missing widgets instead of rendering a placeholder story.
struct GalleryPendingStory {
    message: Option<String>,
}

impl View for GalleryPendingStory {
    fn body(&self, _cx: &Scope) -> AnyView {
        erased(
            AnyView::new(
                Text::new(
                    self.message
                        .clone()
                        .unwrap_or_else(|| "Story unavailable".to_string()),
                )
                .semantic_font(SemanticFontRef::body())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
            )
            .modifier(GalleryPendingFrame),
        )
    }
}

struct GalleryPendingFrame;

impl cove_composition::ViewModifier for GalleryPendingFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_flex_shrink(0.0);
        layout.set_max_width(Some(560.0));
    }
}

struct GalleryPageHeader {
    page: GalleryPage,
}

impl GalleryPageHeader {
    fn view(&self) -> AnyView {
        let entry = self.page.entry();
        erased(
            AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(Text::new(entry.title).semantic_font(SemanticFontRef::title())),
                    AnyView::new(Text::new(entry.summary).semantic_font(SemanticFontRef::body())),
                ])),
            ))
            .padding_edges(24.0, 20.0)
            .modifier(GalleryHeaderFrame),
        )
    }
}

struct GalleryHeaderFrame;

impl cove_composition::ViewModifier for GalleryHeaderFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_flex_shrink(0.0);
    }
}

struct GalleryToolbar {
    options: State<GalleryOptions>,
    text_scale: State<Option<f64>>,
    reset_generation: State<u32>,
}

impl GalleryToolbar {
    fn view(&self) -> AnyView {
        let options = self.options.get();
        let size_select = Select::new(
            Binding::new(
                {
                    let options = self.options.clone();
                    move || options.get().size
                },
                {
                    let options = self.options.clone();
                    move |size: CowSize| {
                        options.update(|current| current.size = size);
                    }
                },
            ),
            vec![
                SelectOption::new(CowSize::Mini, "Mini", true),
                SelectOption::new(CowSize::Small, "Small", true),
                SelectOption::new(CowSize::Regular, "Medium", true),
                SelectOption::new(CowSize::Large, "Large", true),
            ],
        )
        .width(118.0);
        let scale_select = Select::new(
            self.text_scale.binding(),
            TEXT_SCALE_CHOICES
                .iter()
                .map(|(value, label)| SelectOption::new(Some(*value), *label, true))
                .collect(),
        )
        .width(86.0);
        let disabled = Checkbox::from_boolean(Binding::new(
            {
                let options = self.options.clone();
                move || !options.get().is_enabled
            },
            {
                let options = self.options.clone();
                move |is_disabled: bool| {
                    options.update(|current| current.is_enabled = !is_disabled);
                }
            },
        ));
        let reset = {
            let reset_generation = self.reset_generation.clone();
            Button::title("Reset example", move || {
                reset_generation.update(|generation| *generation = generation.saturating_add(1));
            })
        };

        let row = AnyView::new(cove_ui_compose::BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::Wrap,
            12.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(cove_ui_compose::BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        AnyView::new(
                            Text::new("Size")
                                .semantic_font(SemanticFontRef::caption())
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                        AnyView::new(size_select),
                    ])),
                )),
                AnyView::new(cove_ui_compose::BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        AnyView::new(
                            Text::new("Scale")
                                .semantic_font(SemanticFontRef::caption())
                                .semantic_foreground(SemanticColorRef::on_surface_muted()),
                        ),
                        AnyView::new(scale_select),
                    ])),
                )),
                AnyView::new(cove_ui_compose::BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        AnyView::new(disabled),
                        AnyView::new(
                            Text::new("Disabled")
                                .semantic_font(SemanticFontRef::caption())
                                .semantic_foreground(SemanticColorRef::on_surface_variant()),
                        ),
                    ])),
                )),
                erased(AnyView::new(reset).button_style(SecondaryButtonStyle)),
            ])),
        ))
        .padding_edges(24.0, 10.0)
        .modifier(GalleryToolbarFrame);
        let _ = options;
        erased(AnyView::new(row).debug_name("gallery-toolbar"))
    }
}

struct GalleryToolbarFrame;

impl cove_composition::ViewModifier for GalleryToolbarFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_flex_shrink(0.0);
    }
}

/// The radius of a story card; its fill and hairline are semantic colours.
struct GalleryCardModifier;

impl cove_composition::ViewModifier for GalleryCardModifier {
    fn apply_node(&self, node: &Node) {
        node.set_corner_radius(theme_for_node(node).radius.lg);
    }
}

/// One titled story group rendered as a hairline card.
pub struct GalleryStorySection {
    pub title: String,
    pub detail: String,
    pub content: AnyView,
}

impl GalleryStorySection {
    pub fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        content: impl View + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            content: AnyView::new(content),
        }
    }
}

impl View for GalleryStorySection {
    fn body(&self, _cx: &Scope) -> AnyView {
        let card = erased(
            AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                12.0,
                self.content.clone(),
            ))
            .padding(18.0)
            .semantic_background(SemanticColorRef::background())
            .semantic_border(SemanticColorRef::border(), 1.0)
            .modifier(GalleryCardModifier),
        );
        erased(
            AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(
                        Text::new(self.title.clone()).semantic_font(SemanticFontRef::headline()),
                    ),
                    AnyView::new(
                        Text::new(self.detail.clone()).semantic_font(SemanticFontRef::body()),
                    ),
                    card,
                ])),
            ))
            .flex_with(0.0, 0.0, None)
            .frame_min_width(0.0)
            .debug_name(&format!("story-section-{}", self.title)),
        )
    }
}

/// The catalog entry for the page the shell currently shows.
pub fn current_entry(page: GalleryPage) -> GalleryEntry {
    page.entry()
}

/// The size choices the toolbar offers, in declaration order.
pub fn size_choices() -> [CowSize; 4] {
    [
        CowSize::Mini,
        CowSize::Small,
        CowSize::Regular,
        CowSize::Large,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_scale_is_clamped_to_the_supported_range() {
        let view = GalleryView::new(GalleryPage::Introduction, Appearance::Light, 9.0);
        assert_eq!(view.text_scale.get(), Some(2.0));
        let view = GalleryView::new(GalleryPage::Introduction, Appearance::Light, 0.1);
        assert_eq!(view.text_scale.get(), Some(1.0));
    }

    #[test]
    fn navigation_drops_categories_without_matches() {
        let view = GalleryView::new(GalleryPage::Introduction, Appearance::Light, 1.0);
        let sections = view.navigation_sections();
        assert!(!sections.is_empty());
        assert!(view.search.get().string_value().is_empty());
    }

    #[test]
    fn theme_toggle_swaps_the_appearance() {
        let view = GalleryView::new(GalleryPage::Introduction, Appearance::Light, 1.0);
        assert!(!view.is_dark());
        view.toggle_theme();
        assert!(view.is_dark());
    }
}
