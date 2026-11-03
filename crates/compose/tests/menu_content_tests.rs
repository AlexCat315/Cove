//! Translation of `Tests/CoveUIComposeTests/MenuContentTests.swift`.
//!
//! Behaviour checklist:
//! - icons share one lane, rich titles keep the accessible name, and link rows
//!   announce as links with their destination and detail
//! - a rich title paragraph never takes focus or selection from its row
//! - clicking a link row runs its action
//! - type-ahead matches the plain title even when the row draws a rich title,
//!   and empty/nested links are ignored
//! - pointer travel toward an open submenu stays inside the safe triangle
//! - submenu icons reserve the lane in their parent level
//!
use crate::support;

use cove_composition::AnyView;
use cove_core::Rect;
use cove_core::scene::{AccessibilityRole, Point};
use cove_ui_compose::primitives::media::ui_icon_catalog::action;
use cove_ui_compose::primitives::overlays::menu::menu_navigation::MenuTypeaheadRow;
use cove_ui_compose::primitives::overlays::menu::row_content::triangle_contains;
use cove_ui_compose::primitives::overlays::menu::{
    Menu, MenuEntry, MenuItem, MenuItemContent, MenuPointerIntent, MenuSubmenu, MenuTypeahead,
};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::rich::run::{TextRun, TextTraits};

use support::InteractiveScene;

fn all_descendants(
    scene: &InteractiveScene,
    row: &cove_core::scene::Node,
) -> Vec<cove_core::scene::Node> {
    scene
        .nodes()
        .into_iter()
        .filter(|candidate| candidate.id() != row.id() && is_descendant_of(scene, candidate, row))
        .collect()
}

fn is_descendant_of(
    _scene: &InteractiveScene,
    candidate: &cove_core::scene::Node,
    ancestor: &cove_core::scene::Node,
) -> bool {
    let mut current = candidate.parent();
    while let Some(node) = current {
        if node.id() == ancestor.id() {
            return true;
        }
        current = node.parent();
    }
    false
}

#[test]
fn rich_rows_share_an_icon_lane_and_links_announce_as_links() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let opened_for_docs = opened.clone();

        let copy = MenuItem::new(1, "Copy", || {});
        let copy_content = MenuItemContent::new(Some(action::check()), Vec::new(), None, None);

        let paste = MenuItem::new(2, "Paste", || {});

        let docs = MenuItem::new(3, "Documentation", move || {
            opened_for_docs.borrow_mut().push("docs".to_string());
        });
        let docs_content = MenuItemContent::new(
            None,
            vec![
                TextRun::new("Read the "),
                TextRun::with_traits("docs", TextTraits::BOLD),
            ],
            Some("Opens in the browser".to_string()),
            Some("https://example.com/docs".to_string()),
        );

        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            Menu::new(vec![
                MenuEntry::Item(copy.content(copy_content)),
                MenuEntry::Item(paste),
                MenuEntry::Item(docs.content(docs_content)),
            ])
            .width(260.0),
        ))));

        let copy_row = scene
            .labelled("Copy", Some(AccessibilityRole::Button))
            .expect("Copy row");
        let paste_row = scene
            .labelled("Paste", Some(AccessibilityRole::Button))
            .expect("Paste row");
        let docs_row = scene
            .labelled("Documentation", None)
            .expect("Documentation row");

        assert_eq!(
            docs_row.accessibility().map(|s| s.role),
            Some(AccessibilityRole::Link),
            "a link row announces as a link"
        );
        let semantics = docs_row.accessibility().expect("link semantics");
        assert_eq!(
            semantics.help, "https://example.com/docs",
            "the destination is announced as help"
        );
        assert_eq!(
            semantics.value, "Opens in the browser",
            "the detail is announced as the value"
        );

        // Icons share one lane: both titles start at the same x.
        let copy_title = all_descendants(scene, &copy_row)
            .into_iter()
            .find(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label == "Copy")
            })
            .expect("the Copy title materialises");
        let paste_title = all_descendants(scene, &paste_row)
            .into_iter()
            .find(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label == "Paste")
            })
            .expect("the Paste title materialises");
        let copy_x = copy_title.absolute_frame().x;
        let paste_x = paste_title.absolute_frame().x;
        assert!(
            (copy_x - paste_x).abs() < 0.5,
            "an icon row and a plain row share one title lane, {copy_x} vs {paste_x}"
        );

        assert!(
            !all_descendants(scene, &docs_row)
                .iter()
                .any(|node| node.id() != docs_row.id() && node.is_focusable()),
            "rich title paragraphs do not take focus from the row"
        );

        scene.click(&docs_row, 1);
        assert_eq!(
            opened.borrow().clone(),
            vec!["docs".to_string()],
            "clicking the link row runs its action"
        );
    });
}

#[test]
fn type_ahead_matches_the_plain_title_and_ignores_empty_links() {
    let mut typeahead = MenuTypeahead::new();
    let docs = MenuTypeaheadRow {
        id: 4,
        title: "Documentation".to_string(),
    };
    let copy = MenuTypeaheadRow {
        id: 1,
        title: "Copy".to_string(),
    };
    assert_eq!(
        typeahead.destination("d", &[copy.clone(), docs.clone()], None, 0.0),
        Some(4),
        "type-ahead matches the plain title even when the row draws a rich title"
    );

    let empty_link = MenuItemContent::new(None, Vec::new(), None, Some(String::new()));
    assert_eq!(empty_link.link, None, "an empty link string is not a link");
    let nested = MenuItemContent::new(None, vec![TextRun::new("x").link("y")], None, None);
    assert_eq!(
        nested.rich_title.first().and_then(|run| run.link.clone()),
        None,
        "menu titles strip nested link targets"
    );
}

#[test]
fn pointer_travel_toward_a_submenu_stays_inside_the_safe_triangle() {
    let mut intent = MenuPointerIntent::new();
    intent.submenu_frame = Some(Rect::new(200.0, 0.0, 160.0, 200.0));
    intent.record(Point::new(150.0, 40.0));
    intent.record(Point::new(170.0, 60.0));
    assert!(
        intent.is_heading_to_submenu(),
        "a diagonal move toward the open submenu keeps the branch"
    );
    intent.record(Point::new(120.0, 90.0));
    assert!(
        !intent.is_heading_to_submenu(),
        "turning away leaves the safe triangle"
    );

    let mut flipped = MenuPointerIntent::new();
    flipped.submenu_frame = Some(Rect::new(0.0, 0.0, 100.0, 200.0));
    flipped.record(Point::new(160.0, 50.0));
    flipped.record(Point::new(140.0, 70.0));
    assert!(
        flipped.is_heading_to_submenu(),
        "the mirrored approach is also inside the triangle"
    );

    assert!(
        triangle_contains(
            Point::default(),
            Point::new(10.0, 0.0),
            Point::new(0.0, 10.0),
            Point::new(2.0, 2.0)
        ),
        "a point inside the triangle counts"
    );
    assert!(
        !triangle_contains(
            Point::default(),
            Point::new(10.0, 0.0),
            Point::new(0.0, 10.0),
            Point::new(8.0, 8.0)
        ),
        "a point past the hypotenuse does not"
    );
}

#[test]
fn submenu_icons_reserve_the_lane_in_the_parent_level() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let delete = MenuItem::new(6, "Delete", || {});

        scene.install(AnyView::new(LayerRoot::new(AnyView::new(
            Menu::new(vec![
                MenuEntry::Submenu({
                    let mut submenu = MenuSubmenu::new(
                        5,
                        "Share",
                        vec![MenuEntry::Item(MenuItem::new(7, "Mail", || {}))],
                    );
                    submenu.icon = Some(action::external_link());
                    submenu
                }),
                MenuEntry::Item(delete),
            ])
            .width(240.0),
        ))));

        let share_row = scene
            .labelled("Share", Some(AccessibilityRole::Button))
            .expect("Share row");
        let delete_row = scene
            .labelled("Delete", Some(AccessibilityRole::Button))
            .expect("Delete row");

        let images = |row: &cove_core::scene::Node| -> Vec<cove_core::scene::Node> {
            all_descendants(scene, row)
                .into_iter()
                .filter(|node| node.view_tag().is_some_and(|tag| tag.contains("Image")))
                .collect()
        };
        assert!(
            images(&share_row).len() >= 2,
            "a submenu row draws its icon and its chevron, saw {}",
            images(&share_row).len()
        );
        assert!(
            images(&delete_row).is_empty(),
            "a plain row reserves no icon lane, saw {}",
            images(&delete_row).len()
        );
    });
}
