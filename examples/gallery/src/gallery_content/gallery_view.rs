//! The gallery shell: navigation, the page header, the control toolbar, the story
//! outlet and the footer.
//!
//! Translation of `Examples/Sources/GalleryContent/GalleryView.swift`. Story
//! routing, the catalog and the navigation icons live in the sibling modules;
//! this file is the frame around them.
//!
//! The shell is a [`ListDetail`]: the navigation sidebar is the list, the page
//! is the detail. Narrow windows show one of them with a way back; wider ones
//! show both, and a fold or hinge places the split on itself. Both panes keep
//! their state through every change of presentation.
use cove_composition::Scope;
use cove_composition::{AnyView, LayoutDebugExt, View, ViewExt};
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Node, Wrap};
use cove_core::{Binding, Environment, State};
use cove_hosts::desktop::application::composition::AppDisplayHandleHolder;
use cove_ui_compose::adaptive::SurfaceEnvironment;
use cove_ui_compose::animation::{AnimatedVisibility, Transition};
use cove_ui_compose::modifiers::layout::{EdgeInsets, FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::controls::Checkbox;
use cove_ui_compose::primitives::controls::fields::select::{Select, SelectOption};
use cove_ui_compose::primitives::controls::ButtonIconSource;
use cove_ui_compose::primitives::layout::ScrollView;
use cove_ui_compose::primitives::layout::insets::SafeArea;
use cove_ui_compose::primitives::layout::sidebar::{Sidebar, SidebarItem, SidebarSection};
use cove_ui_compose::primitives::media::ui_icon_catalog as icons;
use cove_ui_compose::primitives::navigation::{
    ListDetail, ListDetailPresentation, ListDetailState,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::rich::text_selection::TextSelectionViewExt;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::theme::styles::appearance_modifier::{Appearance, AppearanceViewExt};
use cove_ui_compose::theme::styles::buttons::{
    ButtonStyleViewExt, GhostButtonStyle, SecondaryButtonStyle,
};
use cove_ui_compose::theme::styles::circular_appearance::ThemeTransition;
use cove_ui_compose::theme::theme_for_node;
use cove_ui_compose::theme::tokens::{
    SemanticColorRef, SemanticFontRef, SemanticMotionRef, SemanticStyleViewExt,
};
use cove_ui_compose::theme::{TextScalerReader, TextScalerViewExt};

use crate::gallery_content::catalog::{
    CowSize, GalleryCatalog, GalleryEntry, GalleryOptions, GalleryPage,
};
use crate::gallery_content::routing::StoryDispatch;

/// Erase any view so modifier chains can be stored or returned as `AnyView`.
fn erased(view: impl View + 'static) -> AnyView {
    AnyView::new(view)
}

/// Navigation column width beside a page.
const NAVIGATION_WIDTH: f32 = 240.0;
/// Window width from which the navigation stays beside the page.
const TWO_PANE_WIDTH: f32 = 600.0;
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
    /// In a single pane, whether the page covers the navigation.
    pub showing_page: State<bool>,
    menu_bar: Option<AnyView>,
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
            showing_page: State::new(true),
            menu_bar: None,
        }
    }

    pub fn current_page(&self) -> GalleryPage {
        self.page.get()
    }

    pub fn with_menu_bar(mut self, menu_bar: impl View + 'static) -> Self {
        self.menu_bar = Some(AnyView::new(menu_bar));
        self
    }

    pub fn is_dark(&self) -> bool {
        self.appearance.get() == Appearance::Dark
    }

    pub fn select_page(&self, page: GalleryPage) {
        self.page.set(page);
        self.showing_page.set(true);
    }

    /// Returns to the navigation in a single pane; the page stays selected.
    pub fn show_navigation(&self) {
        self.showing_page.set(false);
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
                        SidebarItem::new(entry.page, entry.title)
                            .icon(entry.page.navigation_icon())
                            .debug_name(format!("sidebar-item-{}", entry.page.raw_value()))
                    })
                    .collect();
                (!items.is_empty()).then(|| SidebarSection::new(category, items).title(category))
            })
            .collect()
    }

    fn search_binding(&self) -> Binding<TextBuffer> {
        self.search.binding()
    }

    fn navigation_state(&self) -> Binding<ListDetailState<GalleryPage>> {
        let page = self.page.clone();
        let showing = self.showing_page.clone();
        Binding::new(
            {
                let page = page.clone();
                let showing = showing.clone();
                move || ListDetailState {
                    selection: Some(page.get()),
                    showing_detail: showing.get(),
                }
            },
            move |value: ListDetailState<GalleryPage>| {
                if let Some(selected) = value.selection
                    && selected != page.get()
                {
                    page.set(selected);
                }
                if value.showing_detail != showing.get() {
                    showing.set(value.showing_detail);
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
            back: None,
            beside_navigation: true,
        };
        let sections = self.navigation_sections();
        let brand = self.navigation_brand();
        let header = self.navigation_header();
        let footer = self.navigation_footer();
        let navigation = ListDetail::new(
            self.navigation_state(),
            move |selection| {
                AnyView::new(
                    AnyView::new(
                        Sidebar::try_new(selection, sections.clone())
                            .expect("unique sidebar IDs")
                            .header(cove_composition::TupleView::new(vec![
                                brand.clone(),
                                header.clone(),
                            ]))
                            .footer(footer.clone()),
                    )
                    .flex_with(1.0, 1.0, Some(0.0))
                    .debug_name("gallery-navigation"),
                )
            },
            move |_page, context| {
                AnyView::new(
                    content
                        .clone()
                        .single_pane(context.is_single_pane.then_some(context.back))
                        .beside_navigation(matches!(
                            context.presentation,
                            ListDetailPresentation::SideBySide { .. }
                        ))
                        .text_selection(true),
                )
            },
        )
        .list_width(NAVIGATION_WIDTH)
        .two_pane_width(TWO_PANE_WIDTH)
        .hairline(false);
        let shell = AnyView::new(navigation)
            .flex_with(1.0, 1.0, Some(0.0))
            .frame(None, None, None, Some(0.0), None, None)
            .debug_name("gallery-shell");
        let shell = if let Some(menu) = &self.menu_bar {
            AnyView::new(
                cove_ui_compose::BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        menu.clone(),
                        AnyView::new(shell),
                    ])),
                )
                .flex_with(1.0, 1.0, Some(0.0)),
            )
        } else {
            AnyView::new(shell)
        };
        let root = AnyView::new(SafeArea::all(LayerRoot::new(shell)))
            .flex_with(1.0, 1.0, Some(0.0))
            .modifier(GalleryShellFrame)
            .modifier(GalleryWindowFill);
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
            DynamicField::state("showing_page", &self.showing_page),
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

    fn navigation_footer(&self) -> AnyView {
        let appearance = self.appearance.get();
        let is_dark = appearance == Appearance::Dark;
        let (theme_tip, theme_icon) = if is_dark {
            ("Switch to light theme", icons::device::sun())
        } else {
            ("Switch to dark theme", icons::device::moon())
        };
        let view = self.clone_handle();
        let theme = AnyView::new(
            Button::icon_default(ButtonIconSource::resource(theme_icon), move || {
                view.toggle_theme()
            })
            .selected(is_dark)
            .tooltip(theme_tip)
            .debug_name("gallery-theme"),
        );

        // The divider toggle reads the published environment, so the launch
        // flag and a runtime click share one source of truth.
        erased(AnyView::new(cove_ui_compose::adaptive::SurfaceReader::new(
            move |environment: &Environment| {
                let separated = environment.title_bar_divider;
                let divider = Button::icon_default(
                    ButtonIconSource::resource(if separated {
                        icons::layout::rows_2()
                    } else {
                        icons::layout::panel_top()
                    }),
                    move || {
                        if let Some(handle) = AppDisplayHandleHolder::current() {
                            handle.set_title_bar_divider(!separated);
                        }
                    },
                )
                .selected(separated)
                .tooltip(if separated {
                    "Show a seamless title bar"
                } else {
                    "Show a title bar divider"
                })
                .debug_name("gallery-title-bar-divider");
                AnyView::new(cove_ui_compose::BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(cove_composition::TupleView::new(vec![
                        theme.clone(),
                        AnyView::new(divider),
                    ])),
                ))
            },
        )))
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
#[derive(Clone)]
struct GalleryPagePane {
    page: State<GalleryPage>,
    options: State<GalleryOptions>,
    reset_generation: State<u32>,
    text_scale: State<Option<f64>>,
    /// Set while the page covers the navigation: returns to it.
    back: Option<std::rc::Rc<dyn Fn()>>,
    /// The navigation sits at the page's leading edge, which then needs no gap.
    beside_navigation: bool,
}

impl GalleryPagePane {
    fn single_pane(mut self, back: Option<std::rc::Rc<dyn Fn()>>) -> Self {
        self.back = back;
        self
    }

    fn beside_navigation(mut self, beside: bool) -> Self {
        self.beside_navigation = beside;
        self
    }
}

impl View for GalleryPagePane {
    fn body(&self, _cx: &Scope) -> AnyView {
        let page = self.page.get();
        let options = self.options.get();
        let generation = self.reset_generation.get();
        let scale = self.text_scale.get().unwrap_or(1.0) as f32;

        let leading_inset = if self.beside_navigation {
            0.0
        } else {
            CARD_INSET
        };
        let header = GalleryPageHeader {
            page,
            back: self.back.clone(),
        };
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
            None => AnyView::new(cove_composition::EmptyView),
        };
        // A page change is a quick cross-fade with an 8pt rise: content
        // swaps, it does not perform. The identity restarts it per navigation.
        let story = erased(
            AnyView::new(AnimatedVisibility::with_motion(
                true,
                Transition::opacity().combined(Transition::offset(0.0, 8.0)),
                Some(SemanticMotionRef::entering()),
                true,
                None,
                story,
            ))
            .id(format!("{}-{generation}", page.raw_value())),
        );
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
            .modifier(GalleryPageFrame)
            .modifier(GalleryContentCard),
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
            .frame_min_width(0.0)
            .padding_insets(EdgeInsets {
                top: CARD_INSET,
                leading: leading_inset,
                bottom: CARD_INSET,
                trailing: CARD_INSET,
            }),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        cove_composition::dynamic_fields!(self; page, options, reset_generation, text_scale)
    }
}

/// Gap between the content card and the window edges.
const CARD_INSET: f32 = 8.0;

/// The window behind the shell: clear over the platform's sidebar material,
/// otherwise the chrome tone the sidebar shares.
struct GalleryWindowFill;

impl cove_composition::ViewModifier for GalleryWindowFill {
    fn apply_node(&self, node: &Node) {
        let translucent = node
            .composition_value(&SurfaceEnvironment::key())
            .translucent_window;
        let colors = theme_for_node(node).colors;
        node.set_background_color((!translucent).then_some(colors.surface_variant));
    }
}

/// The page sits on one soft card: the content fill, large corners, a low
/// shadow and a hairline edge.
struct GalleryContentCard;

impl cove_composition::ViewModifier for GalleryContentCard {
    fn apply_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.background));
        node.set_corner_radius(theme.radius.lg);
        node.set_border_color(Some(theme.colors.divider));
        node.set_border_width(0.5);
        node.set_shadow(theme.elevation.low);
        node.set_clips_to_bounds(true);
    }
}

struct GalleryPageFrame;

impl cove_composition::ViewModifier for GalleryPageFrame {
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

struct GalleryPageHeader {
    page: GalleryPage,
    back: Option<std::rc::Rc<dyn Fn()>>,
}

impl GalleryPageHeader {
    fn view(&self) -> AnyView {
        let entry = self.page.entry();
        let mut lines = Vec::new();
        if let Some(back) = self.back.clone() {
            lines.push(AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                erased(
                    AnyView::new(
                        Button::icon(
                            ButtonIconSource::resource(icons::navigation::arrow_left()),
                            16.0,
                            move || back(),
                        )
                        .tooltip("Components"),
                    )
                    .button_style(GhostButtonStyle)
                    .debug_name("gallery-back"),
                ),
            )));
        }
        lines.push(AnyView::new(
            Text::new(entry.title).semantic_font(SemanticFontRef::title()),
        ));
        lines.push(AnyView::new(
            Text::new(entry.summary).semantic_font(SemanticFontRef::body()),
        ));
        erased(
            AnyView::new(cove_ui_compose::BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(cove_composition::TupleView::new(lines)),
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
        .width(118.0)
        .debug_name("gallery-size");
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
