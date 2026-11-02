//! Translation of `Tests/CoveUIComposeTests/DesignSystemTests.swift`.
//! Application tokens, local overrides, scaled controls and detached portals.

use cove_composition::Scope;
mod support;

use cove_composition::{AnyView, LayoutDebugExt, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Font, FontWeight, State};
use cove_ui_compose::modifiers::style::SemanticStyleViewExt;
use cove_ui_compose::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource};
use cove_ui_compose::primitives::controls::fields::combobox::{Combobox, ComboboxSearch};
use cove_ui_compose::primitives::controls::fields::select::SelectOption;
use cove_ui_compose::primitives::media::icon::Icon;
use cove_ui_compose::primitives::media::ui_icon_catalog::action;
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use cove_ui_compose::primitives::overlays::popover::Popover;
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{
    IconMetrics, IconScaling, IconScalingViewExt, SemanticFontRef, SemanticIconSize, TextScaler,
    TextScalerViewExt, TextStyleToken, Theme, ThemeExtension, ThemeMetric, ThemeViewExt,
};
use cove_ui_compose::{BoxView, TupleView};
use support::InteractiveScene;

#[derive(Clone)]
struct BrandTokens {
    editorial: TextStyleToken,
    gutter: f32,
}

impl ThemeExtension for BrandTokens {
    fn default_value() -> Self {
        Self {
            editorial: TextStyleToken::new(Font::system(19.0, FontWeight::Bold), 28.0, 0.0),
            gutter: 27.0,
        }
    }
}

fn column(children: Vec<AnyView>) -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        Align::FlexStart,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(TupleView::new(children)),
    )
}

fn text(scene: &InteractiveScene, value: &str) -> Node {
    scene
        .first(|node| {
            node.layout_node()
                .and_then(|layout| layout.text_inputs())
                .is_some_and(|inputs| inputs.text == value)
        })
        .expect("text is mounted")
}

fn font_size(node: &Node) -> f32 {
    node.layout_node()
        .expect("text layout")
        .text_measure()
        .expect("text measure")
        .key
        .font
        .size()
}

#[test]
fn custom_roles_are_local_and_extensions_have_value_semantics() {
    let mut app = Theme::default_light();
    app.typography.family = Some("Application family".into());
    app.typography.body = TextStyleToken::new(Font::system(16.0, FontWeight::Regular), 24.0, 0.0);
    let mut tokens = app.extensions.get::<BrandTokens>();
    tokens.gutter = 31.0;
    app.extensions.set(tokens);
    let mut page = app.clone();
    let mut tokens = page.extensions.get::<BrandTokens>();
    tokens.editorial = TextStyleToken::new(
        Font::named("Editorial family", 21.0, FontWeight::Bold),
        29.0,
        0.0,
    );
    page.extensions.set(tokens);
    page.typography.body.font = Font::system(18.0, FontWeight::Regular);
    let role = SemanticFontRef::new(|theme| theme.extensions.get::<BrandTokens>().editorial);
    let gutter = ThemeMetric::new(|theme| theme.extensions.get::<BrandTokens>().gutter);
    assert_eq!(role.resolve(&page).font.size(), 21.0);
    assert_eq!(gutter.resolve(&page), 31.0);
    assert_eq!(
        app.extensions.get::<BrandTokens>().editorial.font.size(),
        19.0
    );
    assert_eq!(
        app.typography.resolved_body().font.family(),
        Some("Application family")
    );
    assert_eq!(page.typography.resolved_body().font.size(), 18.0);
    assert_eq!(
        page.typography
            .body
            .line_height_for(Some(&Font::system(23.0, FontWeight::Regular))),
        24.0 * 23.0 / 18.0
    );
    page.extensions.remove::<BrandTokens>();
    assert_eq!(gutter.resolve(&page), 27.0);
    assert_eq!(gutter.resolve(&app), 31.0);
}

#[test]
fn scaling_uses_the_nearest_scope_and_rejects_invalid_factors() {
    assert_eq!(TextScaler::new(f32::NAN), TextScaler::identity());
    assert_eq!(TextScaler::new(-2.0), TextScaler::identity());
    InteractiveScene::run(900.0, 700.0, |scene| {
        scene.install(
            column(vec![
                AnyView::new(Text::new("Body").semantic_font(SemanticFontRef::body())),
                AnyView::new(
                    Text::new("Local")
                        .semantic_font(SemanticFontRef::body())
                        .text_scaler(TextScaler::new(1.5)),
                ),
            ])
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(2.0)),
        );
        assert_eq!(font_size(&text(scene, "Body")), 28.0);
        assert_eq!(font_size(&text(scene, "Local")), 21.0);
    });
}

#[derive(Clone)]
struct DynamicScale {
    scale: State<f32>,
}

impl View for DynamicScale {
    fn body(&self, _cx: &Scope) -> AnyView {
        let scale = self.scale.clone();
        AnyView::new(LayerRoot::new(
            column(vec![
                AnyView::new(
                    Button::title("Enlarge", move || scale.set(2.0)).debug_name("enlarge"),
                ),
                AnyView::new(Text::new("Dynamic").semantic_font(SemanticFontRef::body())),
                AnyView::new(
                    Popover::new(
                        Text::new("Anchor"),
                        Text::new("Dynamic overlay").semantic_font(SemanticFontRef::body()),
                    )
                    .is_presented(Binding::constant(true)),
                ),
            ])
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(self.scale.get())),
        ))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("scale", &self.scale)]
    }
}

#[test]
fn live_scale_updates_an_already_open_portal() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        let scale = State::new(1.0);
        scene.install(DynamicScale {
            scale: scale.clone(),
        });
        assert_eq!(font_size(&text(scene, "Dynamic")), 14.0);
        scale.set(2.0);
        scene.settle(5, 1.0);
        assert_eq!(font_size(&text(scene, "Dynamic")), 28.0);
        assert_eq!(font_size(&text(scene, "Dynamic overlay")), 28.0);
        assert!(!scene.portal.entries().is_empty());
    });
}

#[test]
fn detached_overlays_inherit_theme_extensions_and_scale() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        let mut theme = Theme::default_light();
        theme.typography.body =
            TextStyleToken::new(Font::system(18.0, FontWeight::Regular), 26.0, 0.0);
        let mut tokens = theme.extensions.get::<BrandTokens>();
        tokens.editorial.font = Font::system(22.0, FontWeight::Regular);
        theme.extensions.set(tokens);
        let popup = Popover::new(
            Text::new("Trigger"),
            column(vec![
                AnyView::new(Text::new("Overlay").semantic_font(SemanticFontRef::body())),
                AnyView::new(
                    Text::new("Extension").semantic_font(SemanticFontRef::new(|t| {
                        t.extensions.get::<BrandTokens>().editorial
                    })),
                ),
            ]),
        )
        .is_presented(Binding::constant(true))
        .width(300.0);
        scene.install(LayerRoot::new(
            popup.theme(theme).text_scaler(TextScaler::new(1.5)),
        ));
        assert_eq!(font_size(&text(scene, "Overlay")), 27.0);
        assert_eq!(font_size(&text(scene, "Extension")), 33.0);
    });
}

#[test]
fn icon_replacements_are_scoped_and_explicit_sizes_win() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        let mut theme = Theme::default_light();
        theme.icons.medium = 29.0;
        theme.icons.replace(&action::check(), action::x());
        assert_eq!(theme.icons.resolve(&action::check()), action::x());
        assert_eq!(
            Theme::default_light().icons.resolve(&action::check()),
            action::check()
        );
        scene.install(
            column(vec![
                AnyView::new(
                    Icon::semantic(action::check(), SemanticIconSize::medium())
                        .debug_name("semantic"),
                ),
                AnyView::new(Icon::new(action::check(), 17.0).debug_name("explicit")),
            ])
            .theme(theme),
        );
        assert_eq!(scene.named("semantic").unwrap().frame().width, 29.0);
        assert_eq!(scene.named("explicit").unwrap().frame().width, 17.0);
    });
}

#[test]
fn icon_metrics_scale_once_for_custom_roles_and_local_policies() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        scene.install(
            column(vec![
                AnyView::new(
                    Icon::semantic(
                        action::check(),
                        SemanticIconSize::new(|t| t.extensions.get::<BrandTokens>().gutter),
                    )
                    .debug_name("custom"),
                ),
                AnyView::new(Icon::new(action::check(), 19.0).debug_name("numeric")),
                AnyView::new(
                    Icon::new(action::check(), 16.0)
                        .icon_scaling(IconScaling::Fixed)
                        .debug_name("fixed"),
                ),
                AnyView::new(
                    Icon::new(action::check(), 16.0)
                        .icon_scaling(IconScaling::Factor(1.5))
                        .debug_name("override"),
                ),
                AnyView::new(
                    Icon::new(action::check(), 16.0)
                        .text_scaler(TextScaler::new(1.25))
                        .debug_name("nearer"),
                ),
            ])
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(2.0)),
        );
        for (name, expected) in [
            ("custom", 54.0),
            ("numeric", 38.0),
            ("fixed", 16.0),
            ("override", 24.0),
            ("nearer", 20.0),
        ] {
            let frame = scene.named(name).unwrap().frame();
            assert_eq!((frame.width, frame.height), (expected, expected), "{name}");
        }
    });
}

#[test]
fn button_icon_sources_share_scale_and_tint() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        let tint = cove_core::Color {
            r: 0.2,
            g: 0.6,
            b: 0.3,
            a: 1.0,
        };
        scene.install(
            column(
                [
                    ButtonIconSource::resource(action::check()),
                    ButtonIconSource::file("/missing-icon.png"),
                    ButtonIconSource::texture(1),
                ]
                .into_iter()
                .enumerate()
                .map(|(i, source)| {
                    AnyView::new(
                        ButtonIcon::new(source, None, Some(tint))
                            .debug_name(&format!("source-{i}")),
                    )
                })
                .collect(),
            )
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(2.0)),
        );
        for i in 0..3 {
            let node = scene.named(&format!("source-{i}")).unwrap();
            assert_eq!(IconMetrics::new(&node).size(16.0), 32.0);
            assert_eq!(node.frame().width, 32.0);
        }
        let mut painted = 0;
        for node in scene.nodes() {
            if let Some(draw) = node.draw() {
                let mut list = cove_core::DrawList::default();
                draw(&mut list, cove_core::Point::default());
                if !list.vertices().is_empty() {
                    painted += 1;
                    assert!(list.vertices().iter().all(|v| v.color == tint.rgba8()));
                }
            }
        }
        assert_eq!(
            painted, 3,
            "resource, file and texture all paint with the explicit tint"
        );
    });
}

#[test]
fn overlay_icon_policy_overrides_the_fixed_theme() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        let mut theme = Theme::default_light();
        theme.icons.scaling = IconScaling::Fixed;
        let popup = Popover::new(
            Text::new("Anchor"),
            Icon::new(action::check(), 16.0).debug_name("overlay-icon"),
        )
        .is_presented(Binding::constant(true))
        .width(200.0)
        .icon_scaling(IconScaling::Factor(1.5));
        scene.install(LayerRoot::new(
            column(vec![
                AnyView::new(Icon::new(action::check(), 16.0).debug_name("theme-icon")),
                AnyView::new(popup),
            ])
            .theme(theme)
            .text_scaler(TextScaler::new(2.0)),
        ));
        assert_eq!(scene.named("theme-icon").unwrap().frame().width, 16.0);
        assert_eq!(scene.named("overlay-icon").unwrap().frame().height, 24.0);
    });
}

#[test]
fn loading_icon_button_keeps_its_name_and_disables_activation() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        scene.install(column(vec![AnyView::new(
            Button::icon(ButtonIconSource::texture(1), 16.0, || {
                panic!("loading button activated")
            })
            .loading(true)
            .tooltip("Saving")
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(2.0)),
        )]));
        let saving = scene
            .labelled("Saving", Some(AccessibilityRole::Button))
            .expect("accessible name");
        assert!(!saving.accessibility().unwrap().state.is_enabled);
        assert_eq!(saving.frame().width, saving.frame().height);
        scene.click(&saving, 1);
    });
}

#[test]
fn menu_rows_follow_scaled_body_typography() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        scene.install(
            Menu::new(vec![
                MenuEntry::Item(MenuItem::new(1, "First", || {}).selected(true)),
                MenuEntry::Item(MenuItem::new(2, "Second", || {})),
            ])
            .width(300.0)
            .max_visible_rows(2)
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(2.0)),
        );
        assert_eq!(font_size(&text(scene, "First")), 28.0);
        let rows = scene.all(|n| n.attachment::<u64>("__menu_item_id").is_some());
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|n| n.frame().height >= 48.0));
    });
}

#[test]
fn combobox_keeps_its_selection_and_accessible_role_when_scaled() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        scene.install(LayerRoot::new(
            Combobox::new(
                Binding::constant(Some(1)),
                vec![
                    SelectOption::new(1, "Paris", true),
                    SelectOption::new(2, "Bern", true),
                ],
            )
            .theme(Theme::default_light())
            .text_scaler(TextScaler::new(2.0)),
        ));
        let trigger = scene
            .labelled("Paris", Some(AccessibilityRole::ComboBox))
            .unwrap();
        assert_eq!(trigger.accessibility().unwrap().value, "Paris");
        scene.click(&trigger, 1);
        scene.settle(5, 1.0);
        assert!(scene.focus.focused().is_some());
        scene
            .dispatcher()
            .dispatch(&cove_platform::InputEvent::TextInput("ber".into()));
        scene.settle(5, 1.0);
        assert!(scene.nodes().iter().any(|n| {
            n.layout_node()
                .and_then(|l| l.text_inputs())
                .is_some_and(|i| i.text == "Bern")
        }));
    });
}

#[test]
fn remote_combobox_opens_and_publishes_results() {
    InteractiveScene::run(900.0, 700.0, |scene| {
        let search = ComboboxSearch::new(|query| {
            Ok(vec![SelectOption::new(
                1,
                if query.is_empty() { "Paris" } else { "Bern" },
                true,
            )])
        });
        scene.install(LayerRoot::new(
            Combobox::search(Binding::<Option<i32>>::constant(None), None, search)
                .theme(Theme::default_light()),
        ));
        let trigger = scene
            .first(|n| {
                n.accessibility()
                    .is_some_and(|a| a.role == AccessibilityRole::ComboBox)
            })
            .unwrap();
        scene.click(&trigger, 1);
        scene.settle(10, 1.0);
        assert!(!scene.portal.entries().is_empty());
    });
}
