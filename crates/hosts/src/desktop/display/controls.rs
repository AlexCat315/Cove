//! Translation of Desktop/Sources/CoveUI/Display/Controls.swift.

use cove_platform::WindowId;
use cove_runtime::PlatformFrameRateMode;
use cove_shell::{FileDialogFilter, WindowChromeHitTest};

/// Runtime presentation overrides installed by the host on the display handle.
#[derive(Clone, Default)]
pub struct PresentationControls {
    pub set_runtime_target_frame_rate: Option<std::rc::Rc<dyn Fn(Option<f64>)>>,
    pub set_runtime_frame_rate_mode: Option<std::rc::Rc<dyn Fn(PlatformFrameRateMode)>>,
    pub set_runtime_vsync_enabled: Option<std::rc::Rc<dyn Fn(bool)>>,
    pub set_title_bar_divider: Option<std::rc::Rc<dyn Fn(bool)>>,
    pub current_runtime_display_refresh_rate: Option<std::rc::Rc<dyn Fn() -> Option<f64>>>,
}

/// Actions the host wires to the main window.
#[derive(Clone, Default)]
pub struct MainWindowControls {
    pub minimize_main_window_action: Option<std::rc::Rc<dyn Fn()>>,
    pub maximize_main_window_action: Option<std::rc::Rc<dyn Fn()>>,
    pub restore_main_window_action: Option<std::rc::Rc<dyn Fn()>>,
    pub hide_main_window_action: Option<std::rc::Rc<dyn Fn()>>,
    pub show_main_window_action: Option<std::rc::Rc<dyn Fn()>>,
    pub close_main_window_action: Option<std::rc::Rc<dyn Fn()>>,
    pub main_window_maximized_query: Option<std::rc::Rc<dyn Fn() -> bool>>,
    pub set_main_window_chrome_hit_test_action:
        Option<std::rc::Rc<dyn Fn(Option<&WindowChromeHitTest>)>>,
    pub set_main_window_title_action: Option<std::rc::Rc<dyn Fn(&str)>>,
}

/// Actions the host wires to any window by identifier.
#[derive(Clone, Default)]
pub struct WindowControls {
    pub minimize_window_by_id_action: Option<std::rc::Rc<dyn Fn(WindowId)>>,
    pub maximize_window_by_id_action: Option<std::rc::Rc<dyn Fn(WindowId)>>,
    pub restore_window_by_id_action: Option<std::rc::Rc<dyn Fn(WindowId)>>,
    pub close_window_by_id_action: Option<std::rc::Rc<dyn Fn(WindowId)>>,
    pub window_maximized_by_id_query: Option<std::rc::Rc<dyn Fn(WindowId) -> bool>>,
    pub show_window_system_menu_action: Option<std::rc::Rc<dyn Fn(WindowId, f32, f32)>>,
}

pub type FolderDialogAction = std::rc::Rc<dyn Fn(Option<&str>, Box<dyn FnOnce(Option<String>)>)>;
pub type FileDialogAction =
    std::rc::Rc<dyn Fn(Option<&str>, &[FileDialogFilter], bool, Box<dyn FnOnce(Vec<String>)>)>;
