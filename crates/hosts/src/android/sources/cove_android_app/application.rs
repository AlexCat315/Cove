//! Translation of Android/CoveAndroidApp/Application.swift.

use super::content::scene::{HostedScene, linked_content};
use super::frame::runtime_options::RuntimeOptions;
use crate::android::contract::launch::SceneLaunch;
use crate::desktop::diagnostics::integration::DevToolsConfiguration;
use cove_graphics::rhi::{Device, DeviceConfig, TextureFormat};
use cove_graphics::ui::NativeDrawListRenderer;
use cove_logging::LogSystem;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// Activity-retained state. Surface loss only releases the swapchain.
pub struct AndroidApplication {
    device: Arc<Device>,
    renderer: Rc<RefCell<NativeDrawListRenderer>>,
    scene: Rc<RefCell<HostedScene>>,
    scale: Cell<f32>,
    runtime: RuntimeOptions,
    configuration: Option<crate::ApplicationConfiguration>,
}

impl AndroidApplication {
    pub fn new(
        scale: f32,
        checkpoint: &str,
        launch: SceneLaunch,
        mut runtime: RuntimeOptions,
    ) -> Result<Rc<Self>, String> {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let content = linked_content();
        let configuration = content.configuration().cloned();
        if let Some(configuration) = &configuration {
            runtime.vsync &= configuration.rendering.vsync;
            runtime.benchmark |= configuration.frames.policy
                == crate::desktop::application::configuration::AppFrameDrivePolicy::Continuous;
            let mut logging = configuration.logging.clone();
            logging
                .apply_process_environment()
                .map_err(|error| error.to_string())?;
            LogSystem::bootstrap(
                Some(&runtime.logging.application),
                logging,
                Some(runtime.logging.run_id.clone()),
            )
            .map_err(|error| error.to_string())?;
        }
        let device_config = configuration
            .as_ref()
            .map(|value| value.rendering.device.clone())
            .unwrap_or(DeviceConfig {
                enable_validation: false,
                ..DeviceConfig::default()
            });
        let device = Arc::new(Device::make(device_config).map_err(|error| error.to_string())?);
        let mut renderer = {
            let device = device.clone();
            device
                .with_frame_session(|| NativeDrawListRenderer::new(device.clone()))
                .map_err(|error| error.to_string())?
        };
        let samples = configuration.as_ref().map_or(1, |configuration| {
            crate::desktop::window::native_renderer::AppWindowRenderSettings::resolve(
                configuration,
                configuration.window.rendering,
            )
            .samples as usize
        });
        renderer
            .configure(TextureFormat::Rgba8Unorm, samples)
            .map_err(|error| error.to_string())?;
        let scene = Rc::new(RefCell::new(HostedScene::with_content(
            checkpoint, launch, scale, content,
        )?));
        let application = Rc::new(Self {
            device,
            renderer: Rc::new(RefCell::new(renderer)),
            scene,
            scale: Cell::new(scale),
            runtime,
            configuration,
        });
        let diagnostics = application
            .runtime
            .devtools
            .clone()
            .map(|launch| DevToolsConfiguration {
                port: 0,
                app_title: launch.app_title.clone(),
                launch: Some(serde_json::json!({
                    "id": launch.id,
                    "token": launch.token,
                    "descriptorPath": launch.descriptor_path,
                    "appTitle": launch.app_title,
                })),
            })
            .or_else(|| {
                application
                    .configuration
                    .as_ref()
                    .and_then(|value| value.diagnostics.dev_tools.clone())
            });
        if let Some(configuration) = diagnostics {
            let weak = Rc::downgrade(&application);
            let schedule: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(application) = weak.upgrade() {
                    application.schedule_diagnostics();
                }
            });
            application
                .scene
                .borrow()
                .content()
                .configure_diagnostics(configuration, schedule)
                .map_err(|error| format!("diagnostics configuration failed: {error}"))?;
        }
        Ok(application)
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn renderer(&self) -> Rc<RefCell<NativeDrawListRenderer>> {
        self.renderer.clone()
    }

    pub fn scene(&self) -> &Rc<RefCell<HostedScene>> {
        &self.scene
    }

    pub fn scale(&self) -> f32 {
        self.scale.get()
    }

    pub fn runtime(&self) -> &RuntimeOptions {
        &self.runtime
    }

    pub fn target_frame_rate(&self) -> Option<f64> {
        self.configuration
            .as_ref()
            .and_then(|value| value.frames.validated_target_rate())
    }

    pub fn clear_color(&self) -> [f32; 4] {
        self.configuration
            .as_ref()
            .map_or([0.025, 0.035, 0.06, 1.0], |value| {
                value
                    .window
                    .rendering
                    .clear_color
                    .unwrap_or(value.rendering.clear_color)
            })
    }

    pub fn update_scale(&self, value: f32) {
        let next = if value.is_finite() && value > 0.0 {
            value
        } else {
            1.0
        };
        if self.scale.get() == next {
            return;
        }
        self.scale.set(next);
        self.scene.borrow().invalidate();
    }

    /// The Java host calls back on the Android UI thread to flush queued
    /// diagnostics work; the queue itself lives in the scene session.
    fn schedule_diagnostics(&self) {
        self.scene.borrow().content().pump_diagnostics();
    }

    pub fn trim_memory(&self) {
        self.scene.borrow_mut().content_mut().trim_memory();
        let device = self.device.clone();
        let renderer = self.renderer.clone();
        let _ = device.with_frame_session(|| {
            device.wait_until_idle()?;
            renderer.borrow_mut().trim_presentation_resources();
            Ok(())
        });
    }

    pub fn shutdown(&self) {
        LogSystem::flush();
    }
}
