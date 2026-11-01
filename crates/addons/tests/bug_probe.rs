//! Temporary probe: confirm three suspected production divergences.
use cove_ui_addons::workspace::{
    WorkspaceAxisSizing, WorkspaceLayoutMetrics, WorkspacePanelSizing, WorkspaceTheme,
};

#[test]
fn probe() {
    // 1. Panel sizing decode must tolerate a missing `height`, as Swift's
    //    `decodeIfPresent` does.
    let decoded = serde_json::from_str::<WorkspacePanelSizing>(r#"{"width":{"minimum":180}}"#);
    println!("1 panel sizing decode: {decoded:?}");

    // 2. Swift `WorkspaceLayoutMetrics()` sets divider = max(1, theme.gutter).
    let theme = WorkspaceTheme::default();
    let metrics = WorkspaceLayoutMetrics::new(&theme, 1.0);
    println!(
        "2 gutter: theme={} metrics.divider={} default.divider={}",
        theme.gutter,
        metrics.divider,
        WorkspaceLayoutMetrics::default().divider
    );

    // 3. Axis sizing normalisation.
    println!(
        "3 clamped: {:?}",
        WorkspaceAxisSizing::new(100.0, Some(10.0), Some(80.0))
    );
}
