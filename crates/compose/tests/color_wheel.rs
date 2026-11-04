//! Colour-wheel behaviour: HSV conversion, angle/point mapping and the pointer
//! interaction that writes them back to the bound colour.
use crate::support;

use cove_core::scene::Point;
use cove_core::{Color, State};
use cove_ui_compose::AnyView;
use cove_ui_compose::LayoutDebugExt;
use cove_ui_compose::primitives::controls::fields::ColorField;
use cove_ui_compose::primitives::controls::fields::{
    ColorFormat, ColorWheel, ColorWheelStyle, Hsv, angle_for_hue, color_from_hsv,
    disk_radius_for_saturation, disk_saturation_for_radius, hsv_from_color, hue_for_angle,
    point_for_sv, sv_for_point,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;

fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn hsv_round_trips_primary_colours() {
    for original in [
        Color::from_rgba8(255, 0, 0, 255),
        Color::from_rgba8(0, 128, 0, 255),
        Color::from_rgba8(32, 64, 200, 255),
    ] {
        let hsv = hsv_from_color(original);
        let restored = color_from_hsv(hsv, original.a);
        assert!(
            close(restored.r, original.r, 0.002),
            "{original:?} → {hsv:?}"
        );
        assert!(
            close(restored.g, original.g, 0.002),
            "{original:?} → {hsv:?}"
        );
        assert!(
            close(restored.b, original.b, 0.002),
            "{original:?} → {hsv:?}"
        );
    }
}

#[test]
fn a_grayscale_has_no_saturation() {
    let hsv = hsv_from_color(Color::from_rgba8(120, 120, 120, 255));
    assert_eq!(hsv.s, 0.0, "equal channels carry no hue information");
}

#[test]
fn red_sits_at_the_top_and_angles_grow_clockwise() {
    use std::f32::consts::{FRAC_PI_2, PI};
    // Red (hue 0) points screen-up, which is −π/2 in the downward-y angle.
    assert!(close(angle_for_hue(0.0), -FRAC_PI_2, 0.0001));
    assert!(close(hue_for_angle(-FRAC_PI_2), 0.0, 0.0001));
    // Quarter turns advance clockwise: right, then down, then left.
    assert!(close(angle_for_hue(0.25), 0.0, 0.0001));
    assert!(close(angle_for_hue(0.5), FRAC_PI_2, 0.0001));
    assert!(close(angle_for_hue(0.75), PI, 0.0001));
    assert!(close(hue_for_angle(PI), 0.75, 0.0001));
}

#[test]
fn the_square_maps_left_to_right_and_top_to_bottom() {
    let side = 100.0;
    let (s, v) = sv_for_point(0.0, 0.0, side);
    assert_eq!((s, v), (0.0, 1.0), "top-left is unsaturated and full value");
    let (s, v) = sv_for_point(100.0, 100.0, side);
    assert_eq!((s, v), (1.0, 0.0), "bottom-right is saturated and black");
    let (s, v) = sv_for_point(75.0, 25.0, side);
    assert!(close(s, 0.75, 0.0001) && close(v, 0.75, 0.0001));
    // Out-of-range points clamp.
    assert_eq!(sv_for_point(-10.0, 200.0, side), (0.0, 0.0));
}

#[test]
fn point_for_sv_inverts_the_mapping() {
    let side = 64.0;
    let (x, y) = point_for_sv(0.25, 0.5, side);
    assert!(close(x, 16.0, 0.0001) && close(y, 32.0, 0.0001));
    assert_eq!(sv_for_point(x, y, side), (0.25, 0.5));
}

#[test]
fn dragging_the_ring_writes_the_hue_under_the_pointer() {
    support::InteractiveScene::run(320.0, 320.0, |scene| {
        let color = State::new(Color::from_rgba8(200, 40, 40, 255));
        scene.install(
            ColorWheel::new(color.binding())
                .diameter(160.0)
                .debug_name("wheel"),
        );
        let wheel = scene
            .named("wheel")
            .expect("the wheel mounts with its debug name");
        let frame = wheel.absolute_frame();
        let center = Point {
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
        };
        let diameter = frame.width.min(frame.height);
        let ring = (diameter * 0.13).clamp(9.0, 26.0);
        let radius = diameter / 2.0 - 1.0 - ring / 2.0;

        // Press red at the top of the ring, then drag to where blue–green
        // (hue 0.5) sits.
        let ring_point = |hue: f32| {
            let angle = angle_for_hue(hue);
            Point {
                x: center.x + angle.cos() * radius,
                y: center.y + angle.sin() * radius,
            }
        };
        scene.drag(ring_point(0.0), ring_point(0.5), 6);
        let hue = hsv_from_color(color.get()).h;
        assert!(close(hue, 0.5, 0.02), "expected hue ~0.5, got {hue}");
    });
}

#[test]
fn dragging_the_square_writes_saturation_and_value() {
    support::InteractiveScene::run(320.0, 320.0, |scene| {
        let color = State::new(Color::from_rgba8(200, 40, 40, 255));
        scene.install(
            ColorWheel::new(color.binding())
                .diameter(160.0)
                .debug_name("wheel"),
        );
        let wheel = scene.named("wheel").expect("the wheel mounts");
        let frame = wheel.absolute_frame();
        let center = Point {
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
        };
        let diameter = frame.width.min(frame.height);
        let ring = (diameter * 0.13).clamp(9.0, 26.0);
        let inner = diameter / 2.0 - 1.0 - ring;
        let side = (inner - 3.0) * std::f32::consts::SQRT_2;
        let left = center.x - side / 2.0;
        let top = center.y - side / 2.0;
        let (dx, dy) = point_for_sv(0.5, 0.25, side);
        let target = Point {
            x: left + dx,
            y: top + dy,
        };
        scene.drag(center, target, 6);
        let hsv = hsv_from_color(color.get());
        assert!(
            close(hsv.s, 0.5, 0.03),
            "expected saturation ~0.5, got {}",
            hsv.s
        );
        assert!(
            close(hsv.v, 0.25, 0.03),
            "expected value ~0.25, got {}",
            hsv.v
        );
    });
}

#[test]
fn an_hsv_drag_keeps_the_alpha() {
    let color = Hsv::new(0.33, 1.0, 1.0).color(0.4);
    assert!(close(color.a, 0.4, 0.0001));
}

#[test]
fn dragging_the_wheel_inside_the_color_field_popover_updates_the_color() {
    support::InteractiveScene::run(600.0, 520.0, |scene| {
        let color = State::new(Color::from_rgba8(200, 40, 40, 255));
        scene.install(LayerRoot::new(AnyView::new(ColorField::new(
            color.binding(),
        ))));
        let trigger = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Choose color"))
            .expect("the color field publishes its trigger");
        scene.click(&trigger, 1);
        scene.settle(40, 1.0 / 60.0);
        let wheel = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Color wheel"))
            .expect("the wheel is mounted in the popover");
        let frame = wheel.absolute_frame();
        let center = Point {
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
        };
        let radius = frame.width.min(frame.height) / 2.0 - 1.0;
        let ring = (frame.width.min(frame.height) * 0.13).clamp(9.0, 26.0);
        let ring_radius = radius - ring / 2.0;
        let point = |hue: f32| {
            let angle = angle_for_hue(hue);
            Point {
                x: center.x + angle.cos() * ring_radius,
                y: center.y + angle.sin() * ring_radius,
            }
        };
        scene.drag(point(0.0), point(0.5), 8);
        let hue = hsv_from_color(color.get()).h;
        assert!(
            close(hue, 0.5, 0.03),
            "dragging the popover wheel changes the bound hue, got {hue}"
        );
    });
}

#[test]
fn a_preset_swatch_sets_the_color() {
    support::InteractiveScene::run(640.0, 560.0, |scene| {
        let color = State::new(Color::from_rgba8(10, 10, 10, 255));
        scene.install(LayerRoot::new(AnyView::new(ColorField::new(
            color.binding(),
        ))));
        let trigger = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Choose color"))
            .expect("the color field publishes its trigger");
        scene.click(&trigger, 1);
        scene.settle(20, 1.0 / 60.0);
        let swatches =
            scene.all(|n| crate::support::debug_name(n).as_deref() == Some("color-swatch"));
        let swatch = swatches
            .get(4)
            .cloned()
            .expect("the palette renders black, grey, light, white and red");
        scene.click(&swatch, 1);
        scene.settle(2, 1.0 / 60.0);
        assert_eq!(
            color.get(),
            Color::from_rgba8(231, 76, 60, 255),
            "clicking a preset writes it to the binding"
        );
    });
}

#[test]
fn committing_a_preset_records_it_as_recent() {
    support::InteractiveScene::run(640.0, 560.0, |scene| {
        let color = State::new(Color::from_rgba8(10, 10, 10, 255));
        let field = ColorField::new(color.binding());
        scene.install(LayerRoot::new(AnyView::new(field.clone())));
        let trigger = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Choose color"))
            .expect("the color field publishes its trigger");
        scene.click(&trigger, 1);
        scene.settle(20, 1.0 / 60.0);
        let swatches =
            scene.all(|n| crate::support::debug_name(n).as_deref() == Some("color-swatch"));
        let red = swatches
            .get(4)
            .cloned()
            .expect("the palette renders its presets");
        scene.click(&red, 1);
        scene.settle(2, 1.0 / 60.0);
        assert!(
            field
                .recent()
                .contains(&Color::from_rgba8(231, 76, 60, 255)),
            "a committed preset joins the recent list"
        );
    });
}

#[test]
fn the_format_toggle_switches_between_hex_and_hsl() {
    support::InteractiveScene::run(640.0, 560.0, |scene| {
        let color = State::new(Color::from_rgba8(88, 126, 231, 255));
        let field = ColorField::new(color.binding());
        scene.install(LayerRoot::new(AnyView::new(field.clone())));
        let trigger = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Choose color"))
            .expect("the color field publishes its trigger");
        scene.click(&trigger, 1);
        scene.settle(20, 1.0 / 60.0);
        assert_eq!(field.color_format(), ColorFormat::Hex);
        let hsl = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "HSL"))
            .expect("the format toggle has an HSL button");
        scene.click(&hsl, 1);
        scene.settle(2, 1.0 / 60.0);
        assert_eq!(field.color_format(), ColorFormat::Hsl);
    });
}

#[test]
fn the_alpha_slider_writes_alpha() {
    support::InteractiveScene::run(640.0, 560.0, |scene| {
        let color = State::new(Color::from_rgba8(88, 126, 231, 255));
        scene.install(LayerRoot::new(AnyView::new(ColorField::new(
            color.binding(),
        ))));
        let trigger = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Choose color"))
            .expect("the color field publishes its trigger");
        scene.click(&trigger, 1);
        scene.settle(20, 1.0 / 60.0);
        let slider = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Alpha"))
            .expect("the alpha slider is in the panel");
        let frame = slider.absolute_frame();
        let start = Point {
            x: frame.x,
            y: frame.y + frame.height / 2.0,
        };
        let end = Point {
            x: frame.x + frame.width * 0.25,
            y: frame.y + frame.height / 2.0,
        };
        scene.drag(start, end, 4);
        assert!(
            close(color.get().a, 0.25, 0.05),
            "dragging the alpha track sets alpha, got {}",
            color.get().a
        );
    });
}

#[test]
fn the_eyedropper_hook_writes_the_sampled_color() {
    support::InteractiveScene::run(640.0, 560.0, |scene| {
        let color = State::new(Color::from_rgba8(10, 10, 10, 255));
        let field = ColorField::new(color.binding())
            .on_sample(|done| done(Color::from_rgba8(1, 2, 3, 255)));
        scene.install(LayerRoot::new(AnyView::new(field.clone())));
        let trigger = scene
            .first(|n| n.accessibility().is_some_and(|s| s.label == "Choose color"))
            .expect("the color field publishes its trigger");
        scene.click(&trigger, 1);
        scene.settle(20, 1.0 / 60.0);
        let dropper = scene
            .first(|n| crate::support::debug_name(n).as_deref() == Some("color-eyedropper"))
            .expect("the host sampler shows the eyedropper button");
        scene.click(&dropper, 1);
        scene.settle(2, 1.0 / 60.0);
        assert_eq!(color.get(), Color::from_rgba8(1, 2, 3, 255));
    });
}

#[test]
fn the_disk_maps_radius_to_saturation() {
    let radius = 50.0;
    assert!(close(disk_saturation_for_radius(0.0, radius), 0.0, 0.0001));
    assert!(close(disk_saturation_for_radius(25.0, radius), 0.5, 0.0001));
    assert!(close(disk_saturation_for_radius(80.0, radius), 1.0, 0.0001));
    assert!(close(disk_radius_for_saturation(0.6, radius), 30.0, 0.0001));
    assert!(close(
        disk_saturation_for_radius(disk_radius_for_saturation(0.4, radius), radius),
        0.4,
        0.0001
    ));
}

#[test]
fn dragging_the_disk_writes_hue_from_angle_and_saturation_from_radius() {
    support::InteractiveScene::run(320.0, 320.0, |scene| {
        let color = State::new(Color::from_rgba8(200, 40, 40, 255));
        scene.install(
            ColorWheel::new(color.binding())
                .style(ColorWheelStyle::Disk)
                .diameter(160.0)
                .debug_name("disk"),
        );
        let wheel = scene.named("disk").expect("the disk wheel mounts");
        let frame = wheel.absolute_frame();
        let center = Point {
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height / 2.0,
        };
        let radius = frame.width.min(frame.height) / 2.0 - 1.0;
        // Hue 0.25 at half saturation.
        let angle = angle_for_hue(0.25);
        let distance = radius * 0.5;
        let target = Point {
            x: center.x + angle.cos() * distance,
            y: center.y + angle.sin() * distance,
        };
        scene.click_at_point(target, 1);
        let hsv = hsv_from_color(color.get());
        assert!(
            close(hsv.h, 0.25, 0.03),
            "expected hue ~0.25, got {}",
            hsv.h
        );
        assert!(
            close(hsv.s, 0.5, 0.04),
            "expected saturation ~0.5, got {}",
            hsv.s
        );
    });
}
