//! Renders every gallery page on each device preset and checks the component
//! adaptation rules of `Docs/adaptive-platform.md` §11.3.
//!
//! Counts per rule, preset and page are compared with
//! `adaptation_baseline.txt`: a count above its baseline fails, and so does a
//! count below it, so the baseline only ever shrinks. Set
//! `COVE_ADAPTATION_AUDIT_WRITE=1` to rewrite it after an intended change.
mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;

use cove_composition::AnyView;
use cove_composition::adaptive::AdaptationMark;
use cove_core::scene::{AccessibilityRole, InputHandlerRole, InteractionRegistry, Node};
use cove_core::{DevicePreset, Environment, InputCapabilities, Rect, WidthClass};
use cove_examples::gallery_content::{GalleryView, catalog::GalleryCatalog};
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;
use support::GalleryScene;

const PRESETS: [&str; 5] = [
    "iphone",
    "android-phone",
    "fold-tabletop",
    "ipad-split",
    "macos",
];

fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/adaptation_baseline.txt")
}

fn environment(preset: &DevicePreset) -> Environment {
    let mut environment = Environment::default();
    environment.viewport.width = preset.size.width;
    environment.viewport.height = preset.size.height;
    preset.environment.apply(&mut environment);
    environment
}

/// A control or text input someone taps: it handles pointer presses itself.
/// Selectable text also takes presses but is read, not tapped.
fn is_interactive(registry: &InteractionRegistry, node: &Node) -> bool {
    node.is_hit_testable()
        && node.is_interaction_enabled()
        && !node.accessibility().is_some_and(|semantics| {
            matches!(
                semantics.role,
                AccessibilityRole::StaticText | AccessibilityRole::Heading
            )
        })
        && registry.handlers(node).pointer_route.is_some_and(|route| {
            matches!(
                route.role,
                InputHandlerRole::Control | InputHandlerRole::TextInput
            )
        })
}

/// Laid out and not collapsed away: hidden panes and closed overlays have no
/// extent.
fn is_shown(frame: Rect) -> bool {
    frame.width > 0.5 && frame.height > 0.5
}

/// Clipped by an ancestor that itself ends inside the window, such as a
/// horizontal scroller: the overflow is scrollable, not lost.
fn clipped_inside(node: &Node, window_width: f32) -> bool {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if ancestor.clips_to_bounds() && ancestor.absolute_frame().max_x() <= window_width + 0.5 {
            return true;
        }
        current = ancestor.parent();
    }
    false
}

fn audit(
    registry: &InteractionRegistry,
    environment: &Environment,
    window: Rect,
    nodes: &[Node],
    counts: &mut BTreeMap<&'static str, usize>,
) {
    let touch = environment.input.contains(InputCapabilities::TOUCH);
    let minimum = environment.minimum_control_extent() - 0.5;
    let handheld = environment.device_class.is_handheld();
    let pointer_desktop = !touch && environment.supports_hover();
    for node in nodes {
        let frame = node.absolute_frame();
        if !is_shown(frame) {
            continue;
        }
        if touch
            && is_interactive(registry, node)
            && (frame.width < minimum || frame.height < minimum)
        {
            *counts.entry("touch-target").or_default() += 1;
            detail("touch-target", node, frame);
        }
        if environment.width_class() == WidthClass::Compact
            && frame.max_x() > window.max_x() + 0.5
            && node
                .parent()
                .is_none_or(|parent| parent.absolute_frame().max_x() <= window.max_x() + 0.5)
            && !clipped_inside(node, window.max_x())
        {
            *counts.entry("horizontal-overflow").or_default() += 1;
            detail("horizontal-overflow", node, frame);
        }
        match AdaptationMark::of(node) {
            Some(AdaptationMark::DesktopOnly) if handheld => {
                *counts.entry("desktop-only").or_default() += 1;
            }
            Some(AdaptationMark::HandheldOnly) if pointer_desktop => {
                *counts.entry("handheld-only").or_default() += 1;
            }
            _ => {}
        }
    }
}

/// `COVE_ADAPTATION_AUDIT_DETAIL=1` prints every violating node.
fn detail(rule: &str, node: &Node, frame: Rect) {
    if std::env::var_os("COVE_ADAPTATION_AUDIT_DETAIL").is_none() {
        return;
    }
    let semantics = node.accessibility();
    eprintln!(
        "{rule}: {:?} {:?} {} at {:.1},{:.1} {:.1}x{:.1}",
        semantics.as_ref().map(|value| value.role),
        semantics.map(|value| value.label).unwrap_or_default(),
        support::debug_name(node).unwrap_or_default(),
        frame.x,
        frame.y,
        frame.width,
        frame.height
    );
}

fn parse(text: &str) -> BTreeMap<String, usize> {
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let (key, count) = line.rsplit_once(' ')?;
            Some((key.to_owned(), count.parse().ok()?))
        })
        .collect()
}

#[test]
fn every_page_follows_the_adaptation_rules_on_every_device() {
    // `COVE_ADAPTATION_AUDIT_ONLY=preset/page` audits one page and skips the
    // baseline comparison, for investigating a count.
    let only = std::env::var("COVE_ADAPTATION_AUDIT_ONLY").ok();
    let mut observed: BTreeMap<String, usize> = BTreeMap::new();
    for name in PRESETS {
        let preset = DevicePreset::named(name).expect("known preset");
        let environment = environment(&preset);
        let window = Rect::new(0.0, 0.0, preset.size.width, preset.size.height);
        for entry in GalleryCatalog::entries() {
            if only
                .as_ref()
                .is_some_and(|only| *only != format!("{name}/{}", entry.page.raw_value()))
            {
                continue;
            }
            let mut counts = BTreeMap::new();
            let surface = environment.clone();
            GalleryScene::run(
                preset.size.width,
                preset.size.height,
                move |_| {
                    AnyView::new(
                        GalleryView::new(entry.page, Appearance::Light, 1.0)
                            .surface_environment(surface),
                    )
                },
                |scene| {
                    scene.settle_n(4);
                    audit(
                        &scene.interactions,
                        &environment,
                        window,
                        &scene.nodes(),
                        &mut counts,
                    );
                },
            );
            for (rule, count) in counts {
                observed.insert(format!("{rule} {name} {}", entry.page.raw_value()), count);
            }
        }
    }

    if only.is_some() {
        eprintln!("{observed:?}");
        return;
    }
    let rendered: String = std::iter::once(
        "# rule preset page count — regenerate with COVE_ADAPTATION_AUDIT_WRITE=1\n".to_owned(),
    )
    .chain(
        observed
            .iter()
            .map(|(key, count)| format!("{key} {count}\n")),
    )
    .collect();
    if std::env::var_os("COVE_ADAPTATION_AUDIT_WRITE").is_some() {
        std::fs::write(baseline_path(), rendered).expect("baseline is writable");
        return;
    }
    let baseline = parse(&std::fs::read_to_string(baseline_path()).unwrap_or_default());
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    for key in baseline.keys().chain(observed.keys()) {
        let before = baseline.get(key).copied().unwrap_or(0);
        let now = observed.get(key).copied().unwrap_or(0);
        if now > before {
            regressions.push(format!("{key}: {before} → {now}"));
        } else if now < before {
            improvements.push(format!("{key}: {before} → {now}"));
        }
    }
    regressions.sort();
    regressions.dedup();
    improvements.sort();
    improvements.dedup();
    assert!(
        regressions.is_empty(),
        "new adaptation violations:\n{}",
        regressions.join("\n")
    );
    assert!(
        improvements.is_empty(),
        "fixed violations; lower the baseline with COVE_ADAPTATION_AUDIT_WRITE=1:\n{}",
        improvements.join("\n")
    );
}
