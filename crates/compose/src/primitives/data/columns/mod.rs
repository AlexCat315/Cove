//! Column presentation shared by the table implementations.
pub mod resize_grip;
pub mod table_presentation;

pub(crate) use resize_grip::ResizeGrip;
pub use table_presentation::TableColumns;

/// The grid's visibility tracks in the picker's own type.
///
/// `TableColumnPlan::tracks` yields the grid's spelling of a track while the
/// picker consumes the shared columns contract, so every table re-expresses the
/// same plan here instead of each keeping its own copy.
pub(crate) fn picker_tracks(
    tracks: &[crate::primitives::layout::grid::ColumnVisibilityTrack],
) -> Vec<crate::primitives::columns::ColumnVisibilityTrack> {
    tracks
        .iter()
        .map(|track| {
            crate::primitives::columns::ColumnVisibilityTrack::new(
                track.id.clone(),
                track.title.clone(),
                match track.visibility {
                    crate::primitives::layout::grid::ColumnVisibility::Required => {
                        crate::primitives::columns::ColumnVisibility::Required
                    }
                    crate::primitives::layout::grid::ColumnVisibility::Automatic { priority } => {
                        crate::primitives::columns::ColumnVisibility::Automatic { priority }
                    }
                },
                track.width,
            )
        })
        .collect()
}
