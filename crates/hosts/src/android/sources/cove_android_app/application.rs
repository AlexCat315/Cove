//! Translation of Android/CoveAndroidApp/Application.swift.

use super::content::scene::HostedScene;
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
}

impl AndroidApplication {
    pub fn new(
        scale: f32,
        checkpoint: &str,
        launch: SceneLaunch,
        runtime: RuntimeOptions,
    ) -> Result<Rc<Self>, String> {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let device = Arc::new(
            Device::make(DeviceConfig {
                enable_validation: false,
                ..DeviceConfig::default()
            })
            .map_err(|error| error.to_string())?,
        );
        let mut renderer = {
            let device = device.clone();
            device
                .with_frame_session(|| NativeDrawListRenderer::new(device.clone()))
                .map_err(|error| error.to_string())?
        };
        renderer
            .configure(TextureFormat::Rgba8Unorm, 1)
            .map_err(|error| error.to_string())?;
        let scene = Rc::new(RefCell::new(HostedScene::new(
            checkpoint,
            launch,
            scale,
            device.clone(),
        )?));
        let application = Rc::new(Self {
            device,
            renderer: Rc::new(RefCell::new(renderer)),
            scene,
            scale: Cell::new(scale),
            runtime,
        });
        if let Some(launch) = application.runtime.devtools.clone() {
            let configuration = DevToolsConfiguration {
                port: 0,
                app_title: launch.app_title.clone(),
                launch: Some(serde_json::json!({
                    "id": launch.id,
                    "token": launch.token,
                    "descriptorPath": launch.descriptor_path,
                    "appTitle": launch.app_title,
                })),
            };
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
