//! Translation of `Tests/CoveUIComposeTests/PackageResourceBundleTests.swift`.
//!
//! Behaviour checklist:
//! - the built-in icon catalog resolves outside a package accessor
//! - the dropdown trigger ships both wider arrow variants, not only the
//!   navigation chevron

use cove_ui_compose::primitives::media::ui_icon_catalog;

#[test]
fn built_in_icon_resolves() {
    assert!(
        ui_icon_catalog::navigation::chevron_down().path().is_some(),
        "Navigation.chevronDown resolves to a bundled file"
    );
    assert!(
        ui_icon_catalog::action::check().path().is_some(),
        "Action.check resolves to a bundled file"
    );
    // Dropdown triggers draw the wider arrow rather than the navigation
    // chevron, so both variants must ship.
    assert!(
        ui_icon_catalog::navigation::dropdown_chevron_down()
            .path()
            .is_some(),
        "Navigation.dropdownChevronDown resolves to a bundled file"
    );
    assert!(
        ui_icon_catalog::navigation::dropdown_chevron_up()
            .path()
            .is_some(),
        "Navigation.dropdownChevronUp resolves to a bundled file"
    );
}
