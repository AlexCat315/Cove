//! With no arguments, produce retained frames without a window, audio device or GPU.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use cove_logging::{Level, LogEvent, LogSystem, LoggingConfiguration};
    use cove_media::{
        gst::{self, prelude::*},
        *,
    };
    use std::time::Duration;
    LogSystem::bootstrap(
        Some("cove-media-headless"),
        LoggingConfiguration::default(),
        None,
    )?;
    gst::initialize()?;
    if let Some(file) = std::env::args_os().nth(1) {
        let context = MediaContext::new(ContextOptions::default())?;
        let player = context.create_player(PlayerOptions {
            audio: AudioOutput::Silent,
            ..PlayerOptions::default()
        })?;
        let request = player.open(MediaSource::file(file))?;
        let result = player.wait_result(request, Duration::from_secs(35))?;
        player.take_result(request)?;
        match result {
            CommandOutcome::Completed { .. } => LogSystem::logger("com.cove.media.example").log(
                &LogEvent::new(Level::Info, format!("{:?}", player.snapshot())),
            ),
            CommandOutcome::Failed(error) => {
                context.shutdown();
                context.wait_closed(Duration::from_secs(5))?;
                return Err(error.into());
            }
            CommandOutcome::Superseded => {
                return Err(MediaError::new(MediaErrorKind::Cancelled, "open superseded").into());
            }
        }
        let request = player.close()?;
        player.wait_result(request, Duration::from_secs(5))?;
        player.take_result(request)?;
        context.shutdown();
        context.wait_closed(Duration::from_secs(5))?;
    } else {
        let output = VideoOutput::new();
        let endpoint = gst::GstVideoEndpoint::new(output.clone())?;
        let pipeline = gst::Pipeline::new();
        let source = gst::ElementFactory::make("videotestsrc")
            .property("num-buffers", 3_i32)
            .build()?;
        pipeline.add_many([&source, endpoint.element()])?;
        source.link(endpoint.element())?;
        pipeline.set_state(gst::State::Playing)?;
        let result = pipeline
            .bus()
            .ok_or("pipeline has no bus")?
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(10),
                &[gst::MessageType::Error, gst::MessageType::Eos],
            );
        pipeline.set_state(gst::State::Null)?;
        match result.as_ref().map(|message| message.view()) {
            Some(gst::MessageView::Eos(_)) => {}
            Some(gst::MessageView::Error(error)) => return Err(error.error().into()),
            _ => return Err("headless pipeline timed out".into()),
        }
        let frame = output.latest().ok_or("pipeline produced no video")?;
        let size: usize = frame.storage.copy_planes()?.iter().map(Vec::len).sum();
        LogSystem::logger("com.cove.media.example").log(&LogEvent::new(
            Level::Info,
            format!(
                "retained frame: {}x{}, pts={:?}, copied_bytes={size}",
                frame.format.width, frame.format.height, frame.pts
            ),
        ));
    }
    LogSystem::flush();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("the native GStreamer example requires a native target".into())
}
