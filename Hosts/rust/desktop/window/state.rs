//! Translation of Desktop/Sources/CoveUI/Window/State.swift.

use cove_shell::WindowChromeHitTest;
use std::time::Instant;

/// Main-window geometry and chrome state owned by the application host.
#[derive(Default)]
pub struct MainWindowState {
    pub drawable_w: u32,
    pub drawable_h: u32,
    pub logical_w: u32,
    pub logical_h: u32,
    pub did_install_root: bool,
    pub last_frame_time: Option<Instant>,
    pub is_vsync_enabled: bool,
    pub main_window_chrome_hit_test_collector: super::chrome::WindowChromeHitTestCollector,
    pub last_main_window_chrome_hit_test: Option<WindowChromeHitTest>,
}

impl MainWindowState {
    pub fn new(is_vsync_enabled: bool) -> Self {
        Self {
            is_vsync_enabled,
            ..Self::default()
        }
    }
}
