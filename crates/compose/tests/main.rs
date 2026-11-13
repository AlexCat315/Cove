//! The compose integration tests, built as one binary so they link once and
//! share one process. Each module is a file in this directory:
//! `cargo test -p cove-ui-compose --test compose menu_button_tests::`.
//!
//! Tests that touch process-wide state take [`support::GlobalTestLock`].

// Historical test fixtures predate the workspace lint gate; keep test-only
// hygiene lints from failing the build.
#![allow(
    dead_code,
    unused_assignments,
    unused_imports,
    clippy::doc_lazy_continuation,
    clippy::double_ended_iterator_last,
    clippy::field_reassign_with_default,
    clippy::same_item_push,
    clippy::single_range_in_vec_init,
    clippy::useless_vec
)]

mod support;

mod accessibility_semantics_tests;
mod action_sheet_tests;
mod adaptive_compact_label;
mod adaptive_dialog_tests;
mod adaptive_disclosure_tests;
mod adaptive_environment;
mod adaptive_navigation_tests;
mod adaptive_sidebar_tests;
mod adaptive_surface_root;
mod adaptive_timeline_tests;
mod advanced_interaction_tests;
mod alert_tests;
mod animated_visibility_tests;
mod animation_integration_tests;
mod animation_value_modifier_tests;
mod app_bar_tests;
mod app_storage;
mod appearance;
mod appearance_reveal_tests;
mod asset_field_tests;
mod asset_ref_field_tests;
mod asset_ref_model;
mod async_image_tests;
mod avatar;
mod avatar_presence;
mod bidi_tests;
mod bool_control_label;
mod bottom_sheet_tests;
mod bracket_matching;
mod button_icon_tests;
mod button_press_recompose_tests;
mod button_scroll_view_tests;
mod button_style_animation_tests;
mod button_style_tests;
mod calendar_adaptive_tests;
mod calendar_keyboard_navigation_tests;
mod calendar_modes_tests;
mod carousel_tests;
mod catalog_interaction_tests;
mod chart_accessibility_tests;
mod chart_axis;
mod chart_interaction_tests;
mod chart_orientation_tests;
mod chart_performance_tests;
mod chart_tests;
mod chart_viewport_tests;
mod chrome_component;
mod chrome_regression_tests;
mod code_editing_ui_tests;
mod color_field_model;
mod color_glyph_resample;
mod color_wheel;
mod combobox_model;
mod combobox_search_tests;
mod command_palette_adaptive_tests;
mod command_palette_footer_tests;
mod command_palette_model;
mod component_wiring_tests;
mod composition_local_compose;
mod composition_local_tests;
mod container_query_tests;
mod control_density_tests;
mod control_discovery_tests;
mod control_motion_tests;
mod cursor_pipeline_tests;
mod custom_layout_tests;
mod custom_layout_visibility_tests;
mod data_table_column_order;
mod data_table_editing;
mod data_table_hosts_model;
mod data_table_interaction;
mod data_table_layout;
mod data_table_layout_order;
mod data_table_model;
mod data_table_row_metrics;
mod data_table_scroll_tests;
mod date_range_picker_tests;
mod date_range_rules;
mod date_range_rules_tests;
mod demo_cross_fade_tests;
mod design_system_tests;
mod disclosure_container_tests;
mod disclosure_group_tests;
mod drag_and_drop_tests;
mod edge_insets_interpolatable;
mod editor_height_tests;
mod editor_interaction_tests;
mod feedback_component_tests;
mod field_layout_tests;
mod file_drop_tests;
mod floating_surface_tests;
mod focus_ring_tests;
mod fold_overlay_tests;
mod four_tree_consistency_tests;
mod grid_columns_tests;
mod grid_layout_tests;
mod grid_modifier_tests;
mod grid_placement_parity;
mod grid_span;
mod gutter_tests;
mod image_file_content_mode_tests;
mod image_pipeline_tests;
mod image_pixel_metrics_tests;
mod indicator_tests;
mod interaction_tests;
mod interactive_scene;
mod json_editor_tests;
mod json_field;
mod json_field_model;
mod keyboard_focus_tests;
mod keyboard_shortcut;
mod keyed_reconciler_tests;
mod label_selection_tests;
mod lazy_grid_tests;
mod list_detail_tests;
mod list_multiple_selection_tests;
mod list_row_tests;
mod list_tree_tests;
mod localization_tests;
mod markdown_tests;
mod masonry_tests;
mod media_player_tests;
mod menu_button_tests;
mod menu_content_tests;
mod menu_hierarchy_tests;
mod menu_pointer_routing_tests;
mod mobile_presentation_tests;
mod modifier_replay_tests;
mod narrow_reflow_tests;
mod native_menu_tests;
mod navigation_drawer_tests;
mod navigation_rail_tests;
mod navigation_stack_tests;
mod nested_geometry_probe;
mod nested_scroll_routing_tests;
mod number_field_tests;
mod numeric_locale;
mod observable_state_tracking_tests;
mod offscreen_reuse;
mod overlay;
mod overlay_scroll_input_tests;
mod package_resource_bundle;
mod popconfirm_tests;
mod popover_tests;
mod portal_layout_tests;
mod portal_scope_tests;
mod primitive_layout_tests;
mod primitive_view_kind_regression;
mod product_component_tests;
mod product_interaction_regression_tests;
mod progress_animation;
mod property_grid_sizing;
mod range_slider_tests;
mod rating_tests;
mod reactive_component_sessions;
mod recomposition_paint;
mod refresh_tests;
mod render_tree_mirror_tests;
mod reorder_model;
mod reorderable_list_tests;
mod responsive_layout_tests;
mod retained_visibility_tests;
mod rich_text_tests;
mod route_and_preload_tests;
mod row_density_tests;
mod row_slot;
mod scale_tests;
mod scratch_geometry;
mod scroll_offset_binding_tests;
mod scroll_physics_tests;
mod scroll_view_input_tests;
mod scrollbar_tests;
mod scrolled_control_click_tests;
mod search_field;
mod search_field_model;
mod sectioned_list_tests;
mod select_combo_role_tests;
mod select_enumeration;
mod selection_field_tests;
mod semantic_motion_tests;
mod sequence_lifecycle_tests;
mod sheet_tests;
mod shortcut_host_tests;
mod sidebar_content_tests;
mod sidebar_hierarchy_tests;
mod sidebar_resize_tests;
mod sidebar_slot_tests;
mod sidebar_tests;
mod signal_scope_tests;
mod single_line_text_tests;
mod slider;
mod slider_axis_tests;
mod snippet_tests;
mod split_button_tests;
mod split_presentation_tests;
mod split_view_panel_tests;
mod spring_interruption_tests;
mod state_propagation_regression_tests;
mod status_bar_panel_truncation_tests;
mod stepper_appearance_tests;
mod style_integration_tests;
mod style_skeleton_tests;
mod surface_reveal_tests;
mod swift_observation_tests;
mod swipe_list_tests;
mod tab_overflow_tests;
mod tab_view_tests;
mod table_columns_tests;
mod table_tests;
mod taffy_layout_api_tests;
mod text_buffer_tests;
mod text_cache;
mod text_divider;
mod text_document_layout_tests;
mod text_editor_appearance_tests;
mod text_field_large_document_tests;
mod text_field_navigation_tests;
mod text_field_tests;
mod text_indent_guides;
mod text_measure_slot;
mod text_scale_rows;
mod text_selection_geometry;
mod text_selection_scope_tests;
mod text_shape_batch_tests;
mod text_shape_cache_tests;
mod text_visual_line_index;
mod theme;
mod theme_cross_fade_component_tests;
mod theme_environment;
mod theme_interaction_regression_tests;
mod theme_paint_invalidation;
mod time_input;
mod time_input_model;
mod time_picker_tests;
mod timeline_group_tests;
mod toast_animation_tests;
mod toggle_tests;
mod toolbar_overflow_tests;
mod tooltip_deduplication_tests;
mod touch_density_tests;
mod tree_behavior;
mod tree_features_tests;
mod tree_reparent_tests;
mod truncation_tests;
mod typography_raster_tests;
mod value_subtree;
mod vec3_field;
mod vec3_field_model;
mod vertical_toolbar_tests;
mod view_builder;
mod view_graph;
mod view_that_fits_tests;
mod viewport_traits_tests;
mod virtual_stack_variable_rows_tests;
mod virtualization_estimate_tests;
mod wheel_picker;
mod wheel_picker_tests;
mod with_animation_completion_tests;
mod with_animation_opacity_tests;
mod with_animation_properties_tests;
mod zstack;

/// Without Cargo's test discovery, an undeclared file would silently not run.
#[test]
fn every_test_file_is_declared() {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    for entry in std::fs::read_dir(directory).expect("the test directory") {
        let path = entry.expect("a directory entry").path();
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("");
        if path.extension().is_some_and(|extension| extension == "rs") && stem != "main" {
            assert!(
                include_str!("main.rs").contains(&format!("\nmod {stem};\n")),
                "declare `mod {stem};` in tests/main.rs"
            );
        }
    }
}
