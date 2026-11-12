use std::{cell::RefCell, rc::Rc, sync::Arc};

use cove_composition::{AnyView, LayoutDebugExt, Scope, View, ViewExt, ViewModifier};
use cove_core::scene::{Align, FlexDirection, Justify, LayoutNode, Wrap};
use cove_core::{Binding, Color, Size};
use cove_imaging::ImageAsset;
use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::modifiers::style::AppearanceExt;
use cove_ui_compose::primitives::controls::{Slider, Toggle};
use cove_ui_compose::primitives::media::{Image, ImageSource};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::tokens::{SemanticColorRef, SemanticFontRef, SemanticStyleViewExt};
use cove_ui_compose::theme::{ControlSize, ControlSizeViewExt};
use cove_ui_compose::{BoxView, TupleView};

use crate::gallery_content::catalog::{CowSize, GalleryOptions};

#[cfg(not(target_arch = "wasm32"))]
mod ray_query;
mod scene;

const INITIAL_LIGHT: u8 = 8;
const LAST_LIGHT: u8 = 16;
const PREVIEW_WIDTH: f32 = 660.0;

#[derive(Clone)]
struct Frame {
    image: Option<Arc<ImageAsset>>,
    backend: Option<&'static str>,
    ray_available: bool,
}

pub(crate) struct RayTracedShadowsStory(pub GalleryOptions);

impl View for RayTracedShadowsStory {
    fn body(&self, cx: &Scope) -> AnyView {
        let light = cx.state(|| f64::from(INITIAL_LIGHT));
        let ray_enabled = cx.state(|| true);
        let position = light.get() as u8;
        let frame = frame(cx, position, ray_enabled.get());
        let can_trace = frame.as_ref().is_some_and(|frame| frame.ray_available);
        let control_size = match self.0.size {
            CowSize::Mini => ControlSize::Mini,
            CowSize::Small => ControlSize::Small,
            CowSize::Regular => ControlSize::Regular,
            CowSize::Large => ControlSize::Large,
        };
        let switch = Toggle::labeled(
            "Ray tracing",
            Binding::new(
                move || can_trace && ray_enabled.get(),
                move |value| ray_enabled.set(value),
            ),
        )
        .enabled(self.0.is_enabled && can_trace)
        .accessibility_label("Ray tracing")
        .debug_name("shadow-ray-tracing");

        let status = match &frame {
            None => "Preparing scene…",
            Some(frame) if frame.image.is_none() => "Scene unavailable",
            Some(frame) => frame.backend.unwrap_or("Rasterization"),
        };
        let mode = BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::SpaceBetween,
            Wrap::Wrap,
            12.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(switch),
                AnyView::new(
                    Text::new(status)
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_muted())
                        .debug_name("shadow-renderer"),
                ),
            ])),
        );

        let controls = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            8.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(mode),
                AnyView::new(Text::new("Light position").semantic_font(SemanticFontRef::body())),
                AnyView::new(
                    Slider::new(light.binding())
                        .range((0.0, f64::from(LAST_LIGHT)))
                        .step(1.0)
                        .enabled(self.0.is_enabled)
                        .accessibility_label("Light position")
                        .frame_min_width(0.0)
                        .debug_name("shadow-light-position"),
                ),
            ])),
        );
        let preview = match frame.and_then(|frame| frame.image) {
            Some(image) => AnyView::new(Image::new(
                ImageSource::Asset(image),
                Size {
                    width: scene::WIDTH as f32,
                    height: scene::HEIGHT as f32,
                },
            )),
            None => AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(cove_composition::EmptyView),
                )
                .background_color(Color::from_rgba8(22, 30, 46, 255)),
            ),
        };

        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                16.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(controls.control_size(control_size)),
                    AnyView::new(
                        preview
                            .modifier(PreviewFrame)
                            .corner_radius(12.0)
                            .accessibility_label("Three blocks casting shadows on a floor")
                            .debug_name("shadow-scene"),
                    ),
                ])),
            )
            .frame(None, None, Some(0.0), None, Some(PREVIEW_WIDTH), None)
            .debug_name("ray-traced-shadows-story"),
        )
    }
}

struct PreviewFrame;

impl ViewModifier for PreviewFrame {
    fn apply_layout(&self, layout: &LayoutNode) {
        layout.set_width_percent(100.0);
        layout.set_height(None);
        layout.set_aspect_ratio(Some(scene::WIDTH as f32 / scene::HEIGHT as f32));
    }
}

fn raster_frame(cx: &Scope, light: u8) -> Frame {
    let cache = cx.remember(|| Rc::new(RefCell::new(None::<(u8, Frame)>)));
    let mut cache = cache.borrow_mut();
    if let Some((position, frame)) = &*cache
        && *position == light
    {
        return frame.clone();
    }
    let geometry = scene::occluders();
    let frame = Frame {
        image: scene::render(
            &geometry,
            &scene::raster_shadow_mask(&geometry, light),
            light,
        )
        .ok(),
        backend: None,
        ray_available: false,
    };
    *cache = Some((light, frame.clone()));
    frame
}

#[cfg(target_arch = "wasm32")]
fn frame(cx: &Scope, light: u8, _trace: bool) -> Option<Frame> {
    Some(raster_frame(cx, light))
}

#[cfg(not(target_arch = "wasm32"))]
fn frame(cx: &Scope, light: u8, trace: bool) -> Option<Frame> {
    use std::{cell::Cell, sync::mpsc};

    let displayed = cx.state(|| None::<Frame>);
    let worker = cx.remember(|| {
        let (requests, pending) = mpsc::channel::<(u8, bool)>();
        let results = displayed.sender();
        std::thread::Builder::new()
            .name("cove-shadow-preview".into())
            .spawn(move || {
                let geometry = scene::occluders();
                let mut gpu = ray_query::RayQuery::available(&geometry);
                while let Ok(first) = pending.recv() {
                    let (light, trace) = pending.try_iter().last().unwrap_or(first);
                    let traced = gpu.as_ref().filter(|_| trace).map(|gpu| gpu.trace(light));
                    let (mask, backend) = match traced {
                        Some(Ok(mask)) => (
                            mask,
                            gpu.as_ref().map(|gpu| match gpu.api() {
                                cove_graphics::rhi::GraphicsApi::Dx12 => "DX12 ray query",
                                cove_graphics::rhi::GraphicsApi::Vulkan => "Vulkan ray query",
                                cove_graphics::rhi::GraphicsApi::Metal => "Metal ray query",
                                cove_graphics::rhi::GraphicsApi::WebGpu => "WebGPU ray query",
                            }),
                        ),
                        other => {
                            if let Some(Err(error)) = other {
                                eprintln!("Shadow preview switched to rasterization: {error}");
                                gpu = None;
                            }
                            (scene::raster_shadow_mask(&geometry, light), None)
                        }
                    };
                    results.set(Some(Frame {
                        image: scene::render(&geometry, &mask, light).ok(),
                        backend,
                        ray_available: gpu.is_some(),
                    }));
                }
            })
            .ok()
            .map(|_| (requests, Rc::new(Cell::new(None))))
    });

    let Some((requests, last)) = worker else {
        return Some(raster_frame(cx, light));
    };
    let request = (light, trace);
    if last.replace(Some(request)) != Some(request) && requests.send(request).is_err() {
        last.set(None);
        return Some(raster_frame(cx, light));
    }
    displayed.get()
}
