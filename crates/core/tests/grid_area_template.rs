//! Named grid areas are the authored form of CSS `grid-template-areas`: a
//! rectangle per name, resolved to explicit 1-based lines.

use cove_core::scene::{GridAreaError, GridAreaTemplate, GridStyle, GridTrack};

#[test]
fn rows_parse_into_rectangles() {
    let template = GridAreaTemplate::new(&["header header", "main side"]).expect("rectangular");
    assert_eq!(template.row_count(), 2);
    assert_eq!(template.column_count(), 2);

    let header = template.area("header").expect("header is named");
    assert_eq!(
        (
            header.column_start,
            header.column_end,
            header.row_start,
            header.row_end
        ),
        (1, 3, 1, 2),
        "header spans both columns and one row"
    );
    let main = template.area("main").expect("main is named");
    assert_eq!(
        (
            main.column_start,
            main.column_end,
            main.row_start,
            main.row_end
        ),
        (1, 2, 2, 3),
        "main is the first cell of the second row"
    );
    assert!(
        template.area("footer").is_none(),
        "an undeclared name has no placement"
    );
}

#[test]
fn unnamed_cells_keep_the_template_extent() {
    let template = GridAreaTemplate::new(&["banner .", ". panel"]).expect("rectangular");
    assert_eq!(template.column_count(), 2);
    let banner = template.area("banner").expect("banner is named");
    assert_eq!((banner.row_end - banner.row_start), 1);
    let panel = template.area("panel").expect("panel is named");
    assert_eq!((panel.column_start, panel.row_start), (2, 2));
}

#[test]
fn ragged_rows_are_rejected() {
    let error = GridAreaTemplate::new(&["a a", "b"]).expect_err("ragged template");
    assert!(
        matches!(
            error,
            GridAreaError::Ragged {
                row: 1,
                expected: 2,
                found: 1
            }
        ),
        "got {error:?}"
    );
}

#[test]
fn split_names_are_rejected() {
    let error =
        GridAreaTemplate::new(&["a . a", ". . ."]).expect_err("a split area is not rectangular");
    assert!(
        matches!(error, GridAreaError::NonRectangular { ref name } if name == "a"),
        "got {error:?}"
    );
    let error = GridAreaTemplate::new(&["a a", "a ."]).expect_err("an L shape is not rectangular");
    assert!(
        matches!(error, GridAreaError::NonRectangular { ref name } if name == "a"),
        "got {error:?}"
    );
}

#[test]
fn empty_templates_are_rejected() {
    assert_eq!(
        GridAreaTemplate::new::<&str>(&[]).expect_err("no rows"),
        GridAreaError::Empty
    );
    assert_eq!(
        GridAreaTemplate::new(&["   "]).expect_err("no cells"),
        GridAreaError::Empty
    );
}

#[test]
fn grid_style_carries_areas() {
    let template = GridAreaTemplate::new(&["a b", "c c"]).expect("rectangular");
    let style = GridStyle {
        areas: Some(template.clone()),
        ..GridStyle::default()
    };
    assert_eq!(style.areas.as_ref(), Some(&template));
    assert_eq!(
        GridStyle::default().areas,
        None,
        "a default style declares no areas"
    );
}

#[test]
fn track_helpers_match_css_breadths() {
    assert_eq!(
        GridTrack::points(240.0),
        GridTrack::length(cove_core::scene::LayoutStyleValue::Points(240.0))
    );
    assert_eq!(
        GridTrack::percent(50.0),
        GridTrack::length(cove_core::scene::LayoutStyleValue::Percent(50.0))
    );
}
