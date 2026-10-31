//! Translation of Desktop/Sources/CoveUI/Window/WindowChrome.swift.
//!
//! The Swift file mixed the immersive title-bar views with the native hit-test
//! collector. The views live in `cove-ui-compose`; this module owns the host
//! half: the per-window ambient, the attachment keys and the incremental
//! collector the host feeds to the platform shell.

use cove_core::Rect;
use cove_core::scene::Node;
use cove_platform::WindowId;
use cove_shell::{ChromeRect, WindowChromeHitTest};
use std::cell::RefCell;

/// Attachment key carrying an authored [`WindowChromeHitTest`] declaration.
pub const CONFIGURATION_KEY: &str = "CoveUI.windowChrome.configuration";
/// Attachment key marking a region the platform may use to drag the window.
pub const DRAG_REGION_KEY: &str = "CoveUI.windowChrome.dragRegion";
/// Attachment key marking a region that must stay interactive (not draggable).
pub const INTERACTIVE_REGION_KEY: &str = "CoveUI.windowChrome.interactiveRegion";

/// How the immersive title bar renders its window controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImmersiveWindowControlStyle {
    #[default]
    Automatic,
    Native,
    Custom,
    Hidden,
}

impl ImmersiveWindowControlStyle {
    /// macOS presents native traffic lights; other platforms draw their own.
    pub fn automatic() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::Native
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::Custom
        }
    }

    pub fn resolve(self) -> Self {
        match self {
            Self::Automatic => Self::automatic(),
            other => other,
        }
    }
}

/// macOS keeps the native resizable window frame under `.hiddenInset`, so it
/// avoids synthetic resize hit-test regions.
pub fn platform_resize_border_width(value: f32) -> f32 {
    #[cfg(target_os = "macos")]
    {
        let _ = value;
        0.0
    }
    #[cfg(not(target_os = "macos"))]
    {
        value
    }
}

/// The window a chrome interaction belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppWindowChromeContext {
    pub window_id: WindowId,
}

impl AppWindowChromeContext {
    pub fn new(window_id: WindowId) -> Self {
        Self { window_id }
    }
}

thread_local! {
    static CURRENT_CHROME_CONTEXT: RefCell<Option<AppWindowChromeContext>> =
        const { RefCell::new(None) };
}

/// Holds the window whose chrome views are currently being installed.
pub struct AppWindowChromeContextHolder;

impl AppWindowChromeContextHolder {
    pub fn current() -> Option<AppWindowChromeContext> {
        CURRENT_CHROME_CONTEXT.with(|slot| *slot.borrow())
    }

    pub fn set_current(context: Option<AppWindowChromeContext>) {
        CURRENT_CHROME_CONTEXT.with(|slot| *slot.borrow_mut() = context);
    }
}

/// Installs the chrome context for the duration of `body` and restores the
/// previous one, mirroring the Swift `withWindowChromeContext`.
pub fn with_window_chrome_context<R>(window_id: Option<WindowId>, body: impl FnOnce() -> R) -> R {
    let previous = AppWindowChromeContextHolder::current();
    if let Some(window_id) = window_id {
        AppWindowChromeContextHolder::set_current(Some(AppWindowChromeContext::new(window_id)));
    }
    struct Restore(Option<AppWindowChromeContext>);
    impl Drop for Restore {
        fn drop(&mut self) {
            AppWindowChromeContextHolder::set_current(self.0);
        }
    }
    let _restore = Restore(previous);
    body()
}

/// Retains only the handful of nodes that participate in native window chrome.
///
/// Geometry is read live from those nodes, while a full-tree scan is needed only
/// when tree membership or order changes.
#[derive(Default)]
pub struct WindowChromeHitTestCollector {
    indexed_root: Option<Node>,
    indexed_structure_version: Option<u64>,
    configuration_nodes: Vec<Node>,
    drag_region_nodes: Vec<Node>,
    interactive_region_nodes: Vec<Node>,
    discovery_pass_count: usize,
}

impl WindowChromeHitTestCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of full-tree scans performed so far. Exposed so a host can assert
    /// a steady frame stayed O(1).
    pub fn discovery_pass_count(&self) -> usize {
        self.discovery_pass_count
    }

    pub fn collect(&mut self, root: &Node) -> Option<WindowChromeHitTest> {
        self.refresh_index_if_needed(root);

        let mut hit_test = self
            .configuration_nodes
            .iter()
            .rev()
            .find_map(|node| node.attachment::<WindowChromeHitTest>(CONFIGURATION_KEY))?;

        hit_test.draggable_rects = Self::rects(&self.drag_region_nodes, DRAG_REGION_KEY);
        hit_test.non_draggable_rects =
            Self::rects(&self.interactive_region_nodes, INTERACTIVE_REGION_KEY);
        Some(hit_test)
    }

    /// One-shot collection with a fresh index, for callers without host state.
    pub fn collect_once(root: &Node) -> Option<WindowChromeHitTest> {
        Self::new().collect(root)
    }

    fn refresh_index_if_needed(&mut self, root: &Node) {
        let version = root.subtree_structure_version();
        let unchanged = self
            .indexed_root
            .as_ref()
            .is_some_and(|indexed| indexed == root)
            && self.indexed_structure_version == Some(version);
        if unchanged {
            return;
        }
        self.indexed_root = Some(root.clone());
        self.indexed_structure_version = Some(version);
        self.configuration_nodes.clear();
        self.drag_region_nodes.clear();
        self.interactive_region_nodes.clear();
        let mut stack = vec![root.clone()];
        while let Some(node) = stack.pop() {
            if node
                .attachment::<WindowChromeHitTest>(CONFIGURATION_KEY)
                .is_some()
            {
                self.configuration_nodes.push(node.clone());
            }
            if node.attachment::<bool>(DRAG_REGION_KEY) == Some(true) {
                self.drag_region_nodes.push(node.clone());
            }
            if node.attachment::<bool>(INTERACTIVE_REGION_KEY) == Some(true) {
                self.interactive_region_nodes.push(node.clone());
            }
            stack.extend(node.children());
        }
        self.discovery_pass_count += 1;
    }

    fn rects(nodes: &[Node], attachment_key: &str) -> Vec<ChromeRect> {
        let mut result = Vec::with_capacity(nodes.len());
        for node in nodes {
            if node.attachment::<bool>(attachment_key) != Some(true) {
                continue;
            }
            // `absolute_frame` rather than a walk over `frame` — these rects are
            // handed to the platform for window dragging, so a copy that lags one
            // geometry rule behind puts the draggable area somewhere the title bar
            // is not.
            let frame: Rect = node.absolute_frame();
            if frame.width <= 0.0 || frame.height <= 0.0 {
                continue;
            }
            result.push(ChromeRect::new(frame.x, frame.y, frame.width, frame.height));
        }
        result
    }
}
