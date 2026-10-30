//! Behaviour tests for asset-kind matching and picker filtering.
//!
//! Translation of `Primitives/Controls/Fields/AssetRefField.swift`. Kind strings
//! are free-form, so keyword matching order and the "empty means anything" rule
//! are the contract.

use std::collections::BTreeSet;

use cove_ui_compose::primitives::controls::fields::{
    field_description, field_tooltip, has_preview, icon_for, picker_options, placeholder_kind,
    AssetIconKind, AssetRef,
};

fn kinds(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn known_kinds_map_to_their_icon() {
    assert_eq!(icon_for(Some("image")), AssetIconKind::Image);
    assert_eq!(icon_for(Some("Texture")), AssetIconKind::Image, "matching is case-insensitive");
    assert_eq!(icon_for(Some("Static Mesh")), AssetIconKind::Mesh, "substring match");
    assert_eq!(icon_for(Some("shader")), AssetIconKind::Material);
    assert_eq!(icon_for(Some("music")), AssetIconKind::Audio);
    assert_eq!(icon_for(Some("mp4")), AssetIconKind::Video);
    assert_eq!(icon_for(Some("prefab")), AssetIconKind::Scene);
    assert_eq!(icon_for(Some("swift")), AssetIconKind::Script);
    assert_eq!(icon_for(Some("json")), AssetIconKind::Text);
    assert_eq!(icon_for(Some("bundle")), AssetIconKind::Package);
}

#[test]
fn an_unknown_or_absent_kind_falls_back_to_the_file_icon() {
    assert_eq!(icon_for(Some("nonsense")), AssetIconKind::File);
    assert_eq!(icon_for(None), AssetIconKind::File);
    assert_eq!(icon_for(Some("")), AssetIconKind::File);
}

#[test]
fn the_first_matching_group_wins() {
    // "image" is in the Image group, which precedes Material — so a kind
    // mentioning both reads as an image.
    assert_eq!(icon_for(Some("image shader")), AssetIconKind::Image);
    // And "shader" alone is Material because Image does not match it.
    assert_eq!(icon_for(Some("shader material")), AssetIconKind::Material);
}

#[test]
fn an_empty_accepted_set_offers_every_option() {
    let options = vec![
        AssetRef::new("a", "A", "image"),
        AssetRef::new("b", "B", "mesh"),
    ];
    let offered = picker_options(&options, &BTreeSet::new());
    assert_eq!(offered.len(), 2, "an empty set means anything");
}

#[test]
fn a_non_empty_accepted_set_filters_by_exact_kind() {
    let options = vec![
        AssetRef::new("a", "A", "image"),
        AssetRef::new("b", "B", "mesh"),
        AssetRef::new("c", "C", "image"),
    ];
    let offered = picker_options(&options, &kinds(&["image"]));
    assert_eq!(offered.len(), 2);
    assert!(offered.iter().all(|option| option.kind == "image"));
}

#[test]
fn the_description_joins_kind_and_subtitle() {
    let with_both = AssetRef::new("a", "A", "image").with_subtitle("Texture 2D");
    assert_eq!(field_description(Some(&with_both)), "image · Texture 2D");

    let kind_only = AssetRef::new("b", "B", "mesh");
    assert_eq!(field_description(Some(&kind_only)), "mesh", "no dangling separator");

    assert_eq!(field_description(None), "");
}

#[test]
fn only_a_single_kind_slot_hints_its_kind() {
    assert_eq!(placeholder_kind(&kinds(&["image"])), Some("image"));
    assert_eq!(placeholder_kind(&kinds(&["image", "mesh"])), None, "ambiguous slots stay generic");
    assert_eq!(placeholder_kind(&BTreeSet::new()), None);
}

#[test]
fn the_tooltip_falls_back_when_there_is_no_kind() {
    assert_eq!(field_tooltip(Some("Static Mesh")), "Static Mesh");
    assert_eq!(field_tooltip(None), "No asset");
}

#[test]
fn a_preview_is_usable_only_when_the_path_is_non_empty() {
    let with = AssetRef::new("a", "A", "image").with_preview_path("/tmp/a.png");
    assert!(has_preview(Some(&with)));
    let empty = AssetRef::new("b", "B", "image").with_preview_path("");
    assert!(!has_preview(Some(&empty)), "an empty path is not a preview");
    let none = AssetRef::new("c", "C", "image");
    assert!(!has_preview(Some(&none)));
    assert!(!has_preview(None));
}
