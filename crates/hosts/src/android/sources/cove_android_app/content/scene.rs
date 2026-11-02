//! Translation of Android/CoveAndroidApp/Content/Scene.swift.
//!
//! `HostedScene` hosts application or showcase content. The content builder is
//! supplied by the platform application through [`HostedSceneContent`], so this
//! host stays free of a dependency on the example products while keeping every
//! host behaviour: launch-mode selection, rendering mode, per-frame typography
//! metrics, checkpoint state and page reporting.

use super::inspection::{
    InputSnapshot, SemanticNode, Snapshot, TypographyStatistics, semantic_nodes,
};
use crate::android::contract::launch::{SceneLaunch, SceneLaunchMode};
use crate::android::integration::scene::{AndroidContentSession, Rendering};
use cove_composition::AnyView;
use cove_core::scene::TextInputSnapshot;
use cove_core::{DrawList, Rect};
use cove_graphics::rhi::Device;
use cove_text::FontAtlas;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Screen-space bounds in the diagnostic snapshot format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Bounds {
    pub fn new(rect: Rect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        }
    }
}

/// Content that a platform application links in for a launch configuration.
///
/// The Android app supplies this from its own content crate; the host only owns
/// the seam. Every method mirrors one branch of the Swift compile-time
/// `#if COVE_ANDROID_SHOWCASE` / `#if COVE_ANDROID_UI` selection.
pub trait HostedSceneContent {
    fn configuration(&self) -> Option<&crate::ApplicationConfiguration> {
        None
    }
    /// Restores authored state from the checkpoint document.
    fn restore_checkpoint(
        &mut self,
        checkpoint: &str,
        mode: SceneLaunchMode,
    ) -> Result<(), String> {
        let _ = (checkpoint, mode);
        Ok(())
    }

    /// Root view for this launch mode, or the reason the mode is unavailable.
    fn root(&mut self, mode: SceneLaunchMode, scale: f32) -> Result<AnyView, String>;

    /// Per-frame typography driver, when the linked content reports metrics.
    fn typography_mut(&mut self) -> Option<&mut dyn TypographyDriver> {
        None
    }

    /// Authored checkpoint values owned by the content.
    fn checkpoint(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// Current page identifier, for gallery-style content.
    fn page(&self) -> Option<String> {
        None
    }
}

/// Typography metrics reported by validation content.
pub trait TypographyDriver {
    fn begin_frame(&mut self);
    fn finish_frame(&mut self);
    fn statistics(&self) -> TypographyStatistics;
    fn atlas(&self) -> Option<FontAtlas> {
        None
    }
    /// Whether the atlas is full and the frame must be prepared again.
    fn atlas_is_full(&self) -> bool {
        false
    }
}

/// The default seam: no external content is linked in, so every launch mode
/// reports the same unsupported-feature errors as the Swift build without a
/// showcase or application content crate.
pub struct UnlinkedContent;

impl HostedSceneContent for UnlinkedContent {
    fn root(&mut self, mode: SceneLaunchMode, _scale: f32) -> Result<AnyView, String> {
        Err(match mode {
            SceneLaunchMode::Gallery | SceneLaunchMode::Counter => {
                "showcase content is not linked into application APKs".to_owned()
            }
            SceneLaunchMode::Game => "game content is hosted by Diffuse".to_owned(),
            SceneLaunchMode::Application => {
                "no application content is linked; declare it with cove_ui::application!".to_owned()
            }
        })
    }
}

/// Application content installed by the linking crate through
/// [`install_application_content`](crate::android::install_application_content).
pub struct ApplicationContent(pub fn() -> AnyView);

impl HostedSceneContent for ApplicationContent {
    fn root(&mut self, mode: SceneLaunchMode, scale: f32) -> Result<AnyView, String> {
        match mode {
            SceneLaunchMode::Application => Ok((self.0)()),
            _ => UnlinkedContent.root(mode, scale),
        }
    }
}

/// One factory call owns both the root and the state captured by its checkpoint.
pub struct DeclaredApplicationContent(pub crate::ApplicationLaunch);

impl HostedSceneContent for DeclaredApplicationContent {
    fn configuration(&self) -> Option<&crate::ApplicationConfiguration> {
        Some(&self.0.configuration)
    }

    fn restore_checkpoint(
        &mut self,
        checkpoint: &str,
        mode: SceneLaunchMode,
    ) -> Result<(), String> {
        if mode != SceneLaunchMode::Application || checkpoint.is_empty() {
            return Ok(());
        }
        let values: BTreeMap<String, String> = serde_json::from_str(checkpoint)
            .map_err(|error| format!("invalid application checkpoint: {error}"))?;
        if let Some(hooks) = &self.0.checkpoint
            && !hooks.restore(&values)
        {
            return Err("application rejected the checkpoint".into());
        }
        Ok(())
    }

    fn root(&mut self, mode: SceneLaunchMode, scale: f32) -> Result<AnyView, String> {
        if mode == SceneLaunchMode::Application {
            Ok(self.0.root.clone())
        } else {
            UnlinkedContent.root(mode, scale)
        }
    }

    fn checkpoint(&self) -> BTreeMap<String, String> {
        self.0
            .checkpoint
            .as_ref()
            .map_or_else(BTreeMap::new, |hooks| hooks.capture())
    }
}

pub(crate) fn linked_content() -> Box<dyn HostedSceneContent> {
    if let Some(factory) = crate::android::application() {
        let launch = factory();
        // The session is built later on this thread, so the backend the launch
        // declares has to be installed here rather than in the desktop runner.
        crate::desktop::diagnostics::integration::set_diagnostics_factory(
            launch.diagnostics.clone(),
        );
        Box::new(DeclaredApplicationContent(launch))
    } else if let Some(factory) = crate::android::application_content() {
        Box::new(ApplicationContent(factory))
    } else {
        Box::new(UnlinkedContent)
    }
}

/// Hosts application or showcase content.
pub struct HostedScene {
    launch: SceneLaunch,
    content: AndroidContentSession,
    drawing: DrawList,
    frames: u64,
    session_id: String,
    content_source: RefCell<Box<dyn HostedSceneContent>>,
}

impl HostedScene {
    pub fn new(
        checkpoint: &str,
        launch: SceneLaunch,
        scale: f32,
        _device: Arc<Device>,
    ) -> Result<Self, String> {
        Self::with_content(checkpoint, launch, scale, linked_content())
    }

    pub fn with_content(
        checkpoint: &str,
        launch: SceneLaunch,
        scale: f32,
        content_source: Box<dyn HostedSceneContent>,
    ) -> Result<Self, String> {
        let mut content_source = content_source;
        let configuration = content_source.configuration().cloned();
        content_source.restore_checkpoint(checkpoint, launch.mode)?;
        let root = content_source.root(launch.mode, scale)?;
        let rendering = if launch.mode == SceneLaunchMode::Counter {
            Rendering::Direct
        } else {
            Rendering::Layers
        };
        let content = match configuration {
            Some(configuration) => {
                AndroidContentSession::with_configuration(scale, rendering, root, configuration)
            }
            None => AndroidContentSession::new(scale, rendering, root),
        };
        Ok(Self {
            launch,
            content,
            drawing: DrawList::default(),
            frames: 0,
            session_id: new_session_id(),
            content_source: RefCell::new(content_source),
        })
    }

    pub fn content(&self) -> &AndroidContentSession {
        &self.content
    }

    pub fn content_mut(&mut self) -> &mut AndroidContentSession {
        &mut self.content
    }

    pub fn launch(&self) -> &SceneLaunch {
        &self.launch
    }

    pub fn list(&self) -> &DrawList {
        &self.drawing
    }

    pub fn size(&self) -> (f32, f32) {
        self.content.logical_size()
    }

    pub fn scale(&self) -> f32 {
        self.content.scale()
    }

    pub fn needs_frame(&self) -> bool {
        self.content.needs_frame()
    }

    pub fn focused_id(&self) -> Option<String> {
        self.content.focused_id()
    }

    pub fn font_atlas(&self) -> FontAtlas {
        let linked = self
            .content_source
            .borrow_mut()
            .typography_mut()
            .and_then(|typography| typography.atlas());
        linked.unwrap_or_else(|| self.content.font_atlas())
    }

    pub fn prepare(&mut self, pixels: (usize, usize), scale: f32, delta_time: f64) {
        let mut source = self.content_source.borrow_mut();
        if let Some(typography) = source.typography_mut() {
            typography.begin_frame();
        }
        drop(source);
        self.drawing = self
            .content
            .prepare((pixels.0 as i32, pixels.1 as i32), scale, delta_time);
        let mut source = self.content_source.borrow_mut();
        if let Some(typography) = source.typography_mut() {
            if typography.atlas_is_full() {
                if let Some(atlas) = typography.atlas() {
                    atlas.reset();
                }
                typography.begin_frame();
                self.drawing = self
                    .content
                    .prepare((pixels.0 as i32, pixels.1 as i32), scale, 0.0);
            }
            typography.finish_frame();
        }
    }

    pub fn did_present(&mut self) {
        self.frames += 1;
        self.content.did_present();
    }

    pub fn invalidate(&self) {
        self.content.invalidate();
    }

    pub fn suspend(&self) {
        self.content.suspend();
    }

    pub fn pointer(&self, kind: i32, x: f32, y: f32) {
        self.content.pointer(kind, x, y);
    }

    pub fn text(&self, string: &str, composing: bool) {
        self.content.text(string, composing);
    }

    pub fn edit(&self, kind: i32, start: i32, end: i32) {
        self.content.edit(kind, start, end);
    }

    pub fn scroll(&self, x: f32, y: f32, delta_x: f32, delta_y: f32) {
        self.content.scroll(x, y, delta_x, delta_y);
    }

    pub fn key(&self, scancode: u32, modifiers: cove_platform::KeyModifiers) {
        self.content.key(scancode, modifiers);
    }

    pub fn has_text_focus(&self) -> bool {
        self.content.editor_snapshot().is_some()
            || self.focused_id().as_deref() == Some("counter.note")
    }

    pub fn editor_snapshot(&self) -> Option<TextInputSnapshot> {
        if let Some(editor) = self.content.editor_snapshot() {
            return Some(editor);
        }
        if !self.has_text_focus() {
            return None;
        }
        let value = self
            .content_source
            .borrow()
            .checkpoint()
            .get("note")
            .cloned()
            .unwrap_or_default();
        let preedit = self.content.preedit();
        let boundary = value.encode_utf16().count() as i32;
        let caret = boundary + preedit.encode_utf16().count() as i32;
        Some(TextInputSnapshot {
            text: format!("{value}{preedit}"),
            selection_start: caret,
            selection_end: caret,
            composition_start: if preedit.is_empty() { -1 } else { boundary },
            composition_end: if preedit.is_empty() { -1 } else { caret },
            ..TextInputSnapshot::default()
        })
    }

    fn typography_statistics(&self) -> TypographyStatistics {
        self.content_source
            .borrow_mut()
            .typography_mut()
            .map(|driver| driver.statistics())
            .unwrap_or_default()
    }

    pub fn snapshot(&self) -> String {
        let inspection = self.content.inspect();
        let controls = inspection
            .controls
            .iter()
            .map(|(name, rect)| (name.clone(), Bounds::new(*rect)))
            .collect::<BTreeMap<_, _>>();
        let (width, height) = self.size();
        let nodes: Vec<SemanticNode> = semantic_nodes(&inspection.semantics);
        let value = Snapshot {
            session: &self.session_id,
            state: self.content_source.borrow().checkpoint(),
            focused: self.focused_id(),
            preedit: self.content.preedit(),
            controls,
            width,
            height,
            scale: self.scale(),
            typography: self.typography_statistics(),
            vertices: self.drawing.vertices().len(),
            indices: self.drawing.indices().len(),
            frames: self.frames,
            mode: self.launch.mode.as_str(),
            page: self.content_source.borrow().page(),
            editor: self.editor_snapshot(),
            nodes,
        };
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned())
    }

    /// The IME and touch routing need only the focused editor, not a traversal
    /// and serialization of every semantic node and diagnostic control bound.
    pub fn input_snapshot(&self) -> String {
        let value = InputSnapshot {
            focused: self.focused_id(),
            editor: self.editor_snapshot(),
        };
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned())
    }
}

fn new_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{nanos:032x}")
}
