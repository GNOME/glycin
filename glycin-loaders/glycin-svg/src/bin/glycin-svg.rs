use std::any::Any;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use gio::{glib, prelude::*};
use glycin_utils::safe_math::*;
use glycin_utils::*;
use gufo_common::image::ImageMetadata;
use rsvg::prelude::*;

/// Current librsvg limit on maximum dimensions. See
/// <https://gitlab.gnome.org/GNOME/librsvg/-/issues/938>
pub const RSVG_MAX_SIZE: u32 = 32_767;

init_main_loader!(ImgDecoder);

#[derive(Default)]
pub struct ImgDecoder {
    thread: Mutex<Option<ImgDecoderDetails>>,
}

pub struct ImgDecoderDetails {
    frame_recv: Receiver<Result<Frame<LocalMemory>, ProcessError>>,
    instr_send: Sender<Instruction>,
    width: u32,
    height: u32,
}

pub struct Instruction {
    total_size: (u32, u32),
    area: Option<rsvg::Rectangle>,
}

pub fn thread<B: ByteData>(
    data: Vec<u8>,
    base_file: Option<gio::File>,
    info_send: Sender<Result<ImageDetails<B>, ProcessError>>,
    frame_send: Sender<Result<Frame<B>, ProcessError>>,
    instr_recv: Receiver<Instruction>,
) {
    let input_stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(data));
    let handle = rsvg::Handle::from_stream_sync(
        &input_stream,
        base_file.as_ref(),
        rsvg::HandleFlags::FLAG_UNLIMITED,
        gio::Cancellable::NONE,
    )
    .expected_error();

    let handle = match handle {
        Ok(handle) => handle,
        Err(err) => {
            info_send.send(Err(err)).unwrap();
            return;
        }
    };

    let (original_width, original_height) = svg_dimensions(&handle);

    let mut image_info = ImageDetails::new(original_width, original_height);

    let intrinsic_dimensions = handle.intrinsic_dimensions();

    image_info.info_format_name = Some(String::from("SVG"));
    image_info.info_dimensions_text = dimensions_text(intrinsic_dimensions);
    image_info.dimensions_inch = dimensions_inch(intrinsic_dimensions);

    info_send.send(Ok(image_info)).unwrap();

    while let Ok(mut instr) = instr_recv.recv() {
        // Overwrite scale width/height with aspect ratio of SVG
        let svg_dimensions = svg_dimensions_float(&handle);
        let scale1 = instr.total_size.0 as f64 / svg_dimensions.0;
        let scale2 = instr.total_size.1 as f64 / svg_dimensions.1;

        let (total_width, total_height) = if scale1 < scale2 {
            (svg_dimensions.0 * scale1, svg_dimensions.1 * scale1)
        } else {
            (svg_dimensions.0 * scale2, svg_dimensions.1 * scale2)
        };

        instr.total_size = (total_width.round() as u32, total_height.round() as u32);

        // librsvg does not currently support larger images
        if instr.total_size.0 > RSVG_MAX_SIZE || instr.total_size.1 > RSVG_MAX_SIZE {
            continue;
        }

        let frame = render(&handle, instr);

        frame_send.send(frame).unwrap();
    }
}

pub fn render<B: ByteData>(
    renderer: &rsvg::Handle,
    instr: Instruction,
) -> Result<Frame<B>, ProcessError> {
    let (total_width, total_height) = instr.total_size;
    let area = instr
        .area
        .unwrap_or_else(|| rsvg::Rectangle::new(0., 0., total_width as f64, total_height as f64));

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
    let texture = B::try_from_slice(&data).expected_error()?;

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
    fn load<B: ByteData, S: Read + Send + 'static>(
        mut stream: S,
        _mime_type: String,
        details: InitializationDetails,
    ) -> Result<(Self, ImageDetails<B>), ProcessError> {
        let mut data = Vec::new();
        stream.read_to_end(&mut data).expected_error()?;

        let (xmp, data) = {
            match gufo_svg::Svg::new(data) {
                Err(err) => (None, err.into_inner()),
                Ok(svg) => (svg.xmp().pop(), svg.into_inner()),
            }
        };

        let (info_send, info_recv) = channel();
        let (frame_send, frame_recv) = channel();
        let (instr_send, instr_recv) = channel();

        let base_file = details
            .base_dir
            .as_ref()
            .map(|x| gio::File::for_path(x).child("placeholder.svg"));

        std::thread::spawn(move || thread(data, base_file, info_send, frame_send, instr_recv));
        let mut image_info = info_recv.recv().unwrap()?;

        image_info.metadata_xmp = xmp.map(LocalMemory::from);

        let decoder = ImgDecoder {
            thread: Mutex::new(Some(ImgDecoderDetails {
                frame_recv,
                instr_send,
                width: image_info.width,
                height: image_info.height,
            })),
        };

        Ok((decoder, image_info.into_other().expected_error()?))
    }

    fn specific_frame<B: ByteData>(
        &mut self,
        frame_request: FrameRequest,
    ) -> Result<Frame<B>, ProcessError> {
        let lock = self.thread.lock().unwrap();
        let thread = lock.as_ref().internal_error()?;

        let width = thread.width;
        let height = thread.height;

        let total_size = frame_request.scale.unwrap_or((width, height));
        let area = if let Some(clip) = frame_request.clip {
            Some(rsvg::Rectangle::new(
                clip.0.into(),
                clip.1.into(),
                clip.2.into(),
                clip.3.into(),
            ))
        } else {
            None
        };

        let instr = Instruction { total_size, area };

        thread.instr_send.send(instr).unwrap();

        let frame = thread.frame_recv.recv().unwrap().expected_error()?;

        Ok(frame.into_other().internal_error()?)
    }
}

pub fn svg_dimensions_float(renderer: &rsvg::Handle) -> (f64, f64) {
    if let Some((width, height)) = renderer.intrinsic_size_in_pixels() {
        (width, height)
    } else {
        let (width, height, vbox) = renderer.intrinsic_dimensions();

        match (width, height, vbox) {
            (width, height, Some(vbox))
                if width.unit() == rsvg::Unit::Percent && height.unit() == rsvg::Unit::Percent =>
            {
                (
                    width.length() * vbox.width(),
                    height.length() * vbox.height(),
                )
            }
            dimensions => {
                eprintln!("Failed to parse SVG dimensions: {dimensions:?}");
                (300., 300.)
            }
        }
    }
}

pub fn svg_dimensions(renderer: &rsvg::Handle) -> (u32, u32) {
    let (width, height) = svg_dimensions_float(renderer);
    (width.round() as u32, height.round() as u32)
}

const fn memory_format() -> MemoryFormat {
    #[cfg(target_endian = "little")]
    {
        MemoryFormat::B8g8r8a8Premultiplied
    }

    #[cfg(target_endian = "big")]
    {
        MemoryFormat::A8r8g8b8Premultiplied
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
