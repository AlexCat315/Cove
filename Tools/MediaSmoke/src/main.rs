//! Independent application: only cove-media is declared, no build script or patch.
use cove_media::{VideoOutput, gst::{self, prelude::*}};

fn retained_frame() -> Result<(), Box<dyn std::error::Error>> {
    gst::initialize()?;
    let output=VideoOutput::new();
    let endpoint=gst::GstVideoEndpoint::new(output.clone())?;
    let pipeline=gst::Pipeline::new();
    let source=gst::ElementFactory::make("videotestsrc").property("num-buffers",3_i32).build()?;
    pipeline.add_many([&source,endpoint.element()])?;
    source.link(endpoint.element())?;
    pipeline.set_state(gst::State::Playing)?;
    let result=pipeline.bus().ok_or("pipeline bus")?.timed_pop_filtered(gst::ClockTime::from_seconds(10),&[gst::MessageType::Error,gst::MessageType::Eos]);
    pipeline.set_state(gst::State::Null)?;
    match result.as_ref().map(|message|message.view()) {
        Some(gst::MessageView::Eos(_))=>{},
        Some(gst::MessageView::Error(error))=>return Err(error.error().into()),
        _=>return Err("pipeline did not reach EOS".into()),
    }
    let frame=output.latest().ok_or("retained frame")?;
    assert!(frame.format.width>0&&frame.format.height>0);
    assert!(frame.storage.copy_planes()?.iter().any(|plane|!plane.is_empty()));
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {retained_frame()}

#[test]
fn an_external_application_can_use_statically_registered_plugins() {
    retained_frame().expect("external consumer pipeline");
}
