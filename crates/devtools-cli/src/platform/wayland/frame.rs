use cove_devtools_protocol::control::Fault;
use image::RgbaImage;
use pipewire as pw;
use pw::spa::{
    self,
    param::{
        ParamType,
        format::FormatProperties,
        video::{VideoFormat, VideoInfoRaw},
    },
    pod::{Pod, Value, serialize::PodSerializer},
    utils::{Direction, Rectangle, SpaTypes},
};
use std::{
    cell::RefCell,
    io::Cursor,
    os::fd::OwnedFd,
    rc::Rc,
    time::{Duration, Instant},
};

fn failure(error: impl std::fmt::Display) -> Fault {
    Fault::new("pipewire_capture", error.to_string())
}

pub(super) fn read(fd: OwnedFd, node: u32) -> Result<RgbaImage, Fault> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopBox::new(None).map_err(failure)?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None).map_err(failure)?;
    let core = context.connect_fd(fd, None).map_err(failure)?;
    let stream = pw::stream::StreamBox::new(
        &core,
        "Cove DevTools",
        pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(failure)?;
    let result = Rc::new(RefCell::new(None));
    let output = result.clone();
    let _listener = stream
        .add_local_listener_with_user_data(VideoInfoRaw::default())
        .param_changed(|_, format, id, param| {
            if id == ParamType::Format.as_raw()
                && let Some(param) = param
            {
                let _ = format.parse(param);
            }
        })
        .process(move |stream, format| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let Some(data) = buffer.datas_mut().first_mut() else {
                return;
            };
            if data.chunk().size() == 0 || output.borrow().is_some() {
                return;
            }
            *output.borrow_mut() = Some(copy(data, format));
        })
        .register()
        .map_err(failure)?;

    let format = spa::pod::object!(
        SpaTypes::ObjectParamFormat,
        ParamType::EnumFormat,
        spa::pod::property!(
            FormatProperties::MediaType,
            Id,
            spa::param::format::MediaType::Video
        ),
        spa::pod::property!(
            FormatProperties::MediaSubtype,
            Id,
            spa::param::format::MediaSubtype::Raw
        ),
        spa::pod::property!(
            FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::BGRx,
            VideoFormat::BGRx,
            VideoFormat::RGBx,
            VideoFormat::BGRA,
            VideoFormat::RGBA
        ),
        spa::pod::property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            Rectangle {
                width: 1920,
                height: 1080
            },
            Rectangle {
                width: 1,
                height: 1
            },
            Rectangle {
                width: 16384,
                height: 16384
            }
        ),
    );
    let pod = PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(format))
        .map_err(failure)?
        .0
        .into_inner();
    let pod = Pod::from_bytes(&pod).ok_or_else(|| failure("Invalid PipeWire format."))?;
    stream
        .connect(
            Direction::Input,
            Some(node),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut [pod],
        )
        .map_err(failure)?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if mainloop
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(Duration::from_millis(100)))
            < 0
        {
            break;
        }
        if let Some(image) = result.borrow_mut().take() {
            return image;
        }
        if let pw::stream::StreamState::Error(message) = stream.state() {
            return Err(failure(message));
        }
    }
    Err(failure(
        "No frame arrived from the selected Wayland stream.",
    ))
}

fn copy(data: &mut spa::buffer::Data, format: &VideoInfoRaw) -> Result<RgbaImage, Fault> {
    let size = format.size();
    let width = size.width as usize;
    let height = size.height as usize;
    let length = width * 4;
    let bgra = match format.format() {
        VideoFormat::BGRA | VideoFormat::BGRx => true,
        VideoFormat::RGBA | VideoFormat::RGBx => false,
        _ => return Err(failure("Unsupported Wayland pixel format.")),
    };
    let offset = data.chunk().offset() as usize;
    let available = data.chunk().size() as usize;
    let stride = usize::try_from(data.chunk().stride()).unwrap_or_default();
    if width == 0 || height == 0 || width.saturating_mul(height) > 40_000_000 || stride < length {
        return Err(failure("Invalid PipeWire frame geometry."));
    }
    let bytes = data
        .data()
        .and_then(|bytes| bytes.get(offset..offset.saturating_add(available)))
        .ok_or_else(|| failure("The compositor did not provide mapped PipeWire pixels."))?;
    let mut output = Vec::with_capacity(length * height);
    for row in 0..height {
        let start = row.saturating_mul(stride);
        let line = bytes
            .get(start..start.saturating_add(length))
            .ok_or_else(|| failure("A PipeWire row exceeded the mapped buffer."))?;
        for pixel in line.chunks_exact(4) {
            let (red, blue) = if bgra {
                (pixel[2], pixel[0])
            } else {
                (pixel[0], pixel[2])
            };
            output.extend_from_slice(&[red, pixel[1], blue, 255]);
        }
    }
    RgbaImage::from_raw(size.width, size.height, output)
        .ok_or_else(|| failure("Invalid captured image."))
}
