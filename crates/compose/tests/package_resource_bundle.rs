//! Translation of `Tests/CoveUIComposeTests/PackageResourceBundleTests.swift`.
//!
//! Behaviour checklist:
//! - the built-in icon catalog resolves outside a package accessor
//! - the dropdown trigger ships both wider arrow variants, not only the
//!   navigation chevron

use cove_ui_compose::primitives::media::ui_icon_catalog;

#[test]
fn built_in_icons_ship_inside_the_binary() {
    // A packaged app carries no source tree, so every built-in icon has to come
    // from bytes compiled into the binary; resolving only through the bundle
    // path left the stars and dropdown arrows blank on device.
    for (name, resource) in [
        (
            "Navigation.chevronDown",
            ui_icon_catalog::navigation::chevron_down(),
        ),
        ("Action.check", ui_icon_catalog::action::check()),
        (
            "Navigation.dropdownChevronDown",
            ui_icon_catalog::navigation::dropdown_chevron_down(),
        ),
        (
            "Navigation.dropdownChevronUp",
            ui_icon_catalog::navigation::dropdown_chevron_up(),
        ),
    ] {
        assert!(
            resource.embedded().is_some(),
            "{name} ships its bytes inside the binary"
        );
    }
    // Dropdown triggers draw the wider arrow rather than the navigation
    // chevron, so the two variants must stay distinct files.
    assert_ne!(
        ui_icon_catalog::navigation::dropdown_chevron_down().name,
        ui_icon_catalog::navigation::chevron_down().name,
        "the dropdown arrow is its own glyph"
    );
}
