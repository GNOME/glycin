use std::io::{Cursor, Read};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use gio::prelude::*;
use glycin_utils::safe_math::*;
use glycin_utils::*;
use rsvg::prelude::*;

/// Current librsvg limit on maximum dimensions. See
/// <https://gitlab.gnome.org/GNOME/librsvg/-/issues/938>
pub const RSVG_MAX_SIZE: u32 = 32_767;

init_main_loader!(ImgDecoder::default());

#[derive(Default)]
pub struct ImgDecoder {
    thread: Mutex<Option<ImgDecoderDetails>>,
}

pub struct ImgDecoderDetails {
    frame_recv: Receiver<Result<Frame, ProcessError>>,
    instr_send: Sender<Instruction>,
    image_info: ImageInfo,
}

pub struct Instruction {
    total_size: (u32, u32),
    area: cairo::Rectangle,
}

pub fn thread(
    stream: UnixStream,
    base_file: Option<gio::File>,
    info_send: Sender<Result<ImageInfo, ProcessError>>,
    frame_send: Sender<Result<Frame, ProcessError>>,
    instr_recv: Receiver<Instruction>,
) {
    let input_stream = unsafe { gio::UnixInputStream::take_fd(stream) };

    let handle = rsvg::Handle::from_stream_sync(
        &input_stream,
        base_file.as_ref(),
        rsvg::HandleFlags::FLAGS_NONE,
        gio::Cancellable::NONE,
    )
    .expected_error();

    let handle = match handle {
        Ok(handle) => handle.unwrap(),
        Err(err) => {
            info_send.send(Err(err)).unwrap();
            return;
        }
    };

    let (original_width, original_height) = svg_dimensions(&handle);

    let mut image_info = ImageInfo::new(original_width, original_height);

    image_info.details.format_name = Some(String::from("SVG"));
    image_info.details.dimensions_text = dimensions_text(handle.intrinsic_dimensions());
    image_info.details.dimensions_inch = dimensions_inch(handle.intrinsic_dimensions());

    info_send.send(Ok(image_info)).unwrap();

    while let Ok(instr) = instr_recv.recv() {
        let (total_width, total_height) = instr.total_size;

        // librsvg does not currently support larger images
        if total_height > RSVG_MAX_SIZE || total_width > RSVG_MAX_SIZE {
            continue;
        }

        let frame = render(&handle, instr);

        frame_send.send(frame).unwrap();
    }
}

pub fn render(renderer: &rsvg::Handle, instr: Instruction) -> Result<Frame, ProcessError> {
    let area = instr.area;
    let (total_width, total_height) = instr.total_size;

    let surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        area.width() as i32,
        area.height() as i32,
    )
    .expected_error()?;

    let context = cairo::Context::new(&surface).expected_error()?;

    renderer
        .render_document(
            &context,
            &rsvg::Rectangle::new(
                -area.x(),
                -area.y(),
                total_width as f64,
                total_height as f64,
            ),
        )
        .expected_error()?;

    drop(context);

    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride() as usize;

    let data = surface.take_data().internal_error()?.to_vec();

    let mut memory = SharedMemory::new(data.len().try_u64()?).expected_error()?;

    Cursor::new(data).read_exact(&mut memory).expected_error()?;
    let texture = memory.into_binary_data();

    let mut frame = Frame::new(
        width.try_u32()?,
        height.try_u32()?,
        memory_format(),
        texture,
    )?;

    frame.stride = stride.try_u32()?;

    Ok(frame)
}

impl LoaderImplementation for ImgDecoder {
    fn init(
        &self,
        stream: UnixStream,
        _mime_type: String,
        details: InitializationDetails,
    ) -> Result<ImageInfo, ProcessError> {
        let (info_send, info_recv) = channel();
        let (frame_send, frame_recv) = channel();
        let (instr_send, instr_recv) = channel();

        let base_file = details
            .base_dir
            .as_ref()
            .map(|x| gio::File::for_path(x).child("placeholder.svg"));

        std::thread::spawn(move || thread(stream, base_file, info_send, frame_send, instr_recv));
        let image_info = info_recv.recv().unwrap()?;

        *self.thread.lock().unwrap() = Some(ImgDecoderDetails {
            frame_recv,
            instr_send,
            image_info: image_info.clone(),
        });

        Ok(image_info)
    }

    fn frame(&self, frame_request: FrameRequest) -> Result<Frame, ProcessError> {
        let lock = self.thread.lock().unwrap();
        let thread = lock.as_ref().internal_error()?;

        let image_info = &thread.image_info;
        let width = image_info.width;
        let height = image_info.height;

        let total_size = frame_request.scale.unwrap_or((width, height));
        let area = if let Some(clip) = frame_request.clip {
            cairo::Rectangle::new(clip.0.into(), clip.1.into(), clip.2.into(), clip.3.into())
        } else {
            cairo::Rectangle::new(0., 0., total_size.0.into(), total_size.1.into())
        };

        let instr = Instruction { total_size, area };

        thread.instr_send.send(instr).unwrap();

        thread.frame_recv.recv().unwrap()
    }
}

pub fn svg_dimensions(renderer: &rsvg::Handle) -> (u32, u32) {
    if let Some((width, height)) = renderer.intrinsic_size_in_pixels() {
        (width.ceil() as u32, height.ceil() as u32)
    } else {
        let (width, height, vbox) = renderer.intrinsic_dimensions();

        match (width, height, vbox) {
            (width, height, Some(vbox))
                if width.unit() == rsvg::Unit::Percent && height.unit() == rsvg::Unit::Percent =>
            {
                (
                    (width.length() * vbox.width()).ceil() as u32,
                    (height.length() * vbox.height()).ceil() as u32,
                )
            }
            dimensions => {
                eprintln!("Failed to parse SVG dimensions: {dimensions:?}");
                (300, 300)
            }
        }
    }
}

const fn memory_format() -> MemoryFormat {
    #[cfg(target_endian = "little")]
    {
        MemoryFormat::B8g8r8a8
    }

    #[cfg(target_endian = "big")]
    {
        MemoryFormat::A8r8g8b8
    }
}

pub fn dimensions_text(
    intrisic_dimensions: (rsvg::Length, rsvg::Length, Option<rsvg::Rectangle>),
) -> Option<String> {
    let width = intrisic_dimensions.0;
    let height = intrisic_dimensions.1;

    if width.unit() == rsvg::Unit::Px && height.unit() == rsvg::Unit::Px {
        None
    } else {
        // Percent is not stored as percentile
        let width_factor = if width.unit() == rsvg::Unit::Percent {
            100.
        } else {
            1.
        };
        let height_factor = if height.unit() == rsvg::Unit::Percent {
            100.
        } else {
            1.
        };

        // Only show two digits
        let width_n = (width.length() * width_factor * 100.).round() / 100.;
        let height_n = (height.length() * height_factor * 100.).round() / 100.;

        let width_unit = width.unit();
        let height_unit = height.unit();

        Some(format!(
            "{width_n}\u{202F}{width_unit} \u{D7} {height_n}\u{202F}{height_unit}"
        ))
    }
}

pub fn dimensions_inch(
    intrisic_dimensions: (rsvg::Length, rsvg::Length, Option<rsvg::Rectangle>),
) -> Option<(f64, f64)> {
    let width = intrisic_dimensions.0;
    let height = intrisic_dimensions.1;

    if let (Some(w), Some(h)) = (dimension_inch(width), dimension_inch(height)) {
        Some((w, h))
    } else {
        None
    }
}

pub fn dimension_inch(length: rsvg::Length) -> Option<f64> {
    match length.unit() {
        rsvg::Unit::In => Some(length.length()),
        rsvg::Unit::Cm => Some(length.length() / 2.54),
        rsvg::Unit::Mm => Some(length.length() / 25.4),
        rsvg::Unit::Pt => Some(length.length() * 72.),
        rsvg::Unit::Pc => Some(length.length() / 12. * 72.),
        _ => None,
    }
}
