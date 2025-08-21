use std::ffi::{OsStr, OsString};

use gio::glib;
use gio::prelude::*;
use glycin::MemoryFormatSelection;
use image::imageops;

const SCALE_FILTER1: imageops::FilterType = imageops::FilterType::Nearest;
const SCALE_FILTER2: imageops::FilterType = imageops::FilterType::Triangle;

fn main() {
    let app = gio::Application::new(None, gio::ApplicationFlags::HANDLES_COMMAND_LINE);

    app.add_main_option(
        "input",
        glib::Char::from(b'i'),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        "Input URL",
        Some("INPUT_URL"),
    );

    app.add_main_option(
        "output",
        glib::Char::from(b'o'),
        glib::OptionFlags::NONE,
        glib::OptionArg::Filename,
        "Output path",
        Some("OUTPUT_PATH"),
    );

    app.add_main_option(
        "size",
        glib::Char::from(b's'),
        glib::OptionFlags::NONE,
        glib::OptionArg::Int,
        "Maximum thumbnail size",
        Some("SIZE"),
    );

    app.connect_command_line(move |_, args| {
        let args_dict = args.options_dict();

        let Some(input_uri) = args_dict.lookup::<String>("input").unwrap() else {
            eprintln!("Error: Input URI not supplied.");
            return glib::ExitCode::from(2);
        };

        let Some(output_path) = args_dict.lookup::<OsString>("output").unwrap() else {
            eprintln!("Error: Output path not supplied.");
            return glib::ExitCode::from(2);
        };

        let Some(thumbnail_size) = args_dict.lookup::<i32>("size").unwrap() else {
            eprintln!("Error: Size not supplied.");
            return glib::ExitCode::from(2);
        };

        if let Err(err) = x(&input_uri, &output_path, thumbnail_size.try_into().unwrap()) {
            eprintln!("Glycin Thumbnailer: {err}");
            glib::ExitCode::from(1)
        } else {
            glib::ExitCode::from(0)
        }
    });

    app.run();
}

fn x(
    input_uri: &str,
    output_path: &OsStr,
    thumbnail_size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let input_file = gio::File::for_uri(input_uri);

    let mut loader = glycin::Loader::new(input_file.clone());

    // Disable sandbox since thumbnailers run in their own sandbox
    loader.sandbox_selector(glycin::SandboxSelector::NotSandboxed);
    loader.accepted_memory_formats(MemoryFormatSelection::R8g8b8 | MemoryFormatSelection::R8g8b8a8);

    let image = glib::MainContext::default().block_on(loader.load())?;
    let frame_request = glycin::FrameRequest::new().scale(thumbnail_size, thumbnail_size);
    let frame = glib::MainContext::default().block_on(image.specific_frame(frame_request))?;

    let out_file = std::fs::File::create(output_path)?;
    let buf_writer = &mut std::io::BufWriter::new(out_file);

    // Reduce max size to thumbnail size
    let scale = thumbnail_size as f32 / u32::max(frame.width(), frame.height()) as f32;
    // Ensure the image is not scaled up
    let scale = f32::min(1., scale);

    let thumbnail_width = (frame.width() as f32 * scale).round() as u32;
    let thumbnail_height = (frame.height() as f32 * scale).round() as u32;

    let buf;
    let color;

    match frame.memory_format() {
        glycin::MemoryFormat::R8g8b8 => {
            buf = resize::<image::Rgb<u8>>(&frame, thumbnail_width, thumbnail_height);
            color = png::ColorType::Rgb;
        }
        glycin::MemoryFormat::R8g8b8a8 => {
            buf = resize::<image::Rgba<u8>>(&frame, thumbnail_width, thumbnail_height);
            color = png::ColorType::Rgba;
        }
        unexpected_format => unreachable!("Unexpected memory format: {unexpected_format:?}"),
    };

    let mut encoder = png::Encoder::new(buf_writer, thumbnail_width, thumbnail_height);
    encoder.set_color(color);

    let mut writer = encoder.write_header()?;

    writer.write_image_data(&buf)?;

    Ok(())
}

fn resize<T: image::Pixel<Subpixel = u8> + 'static>(
    frame: &glycin::Frame,
    thumbnail_width: u32,
    thumbnail_height: u32,
) -> Vec<u8> {
    let img =
        image::ImageBuffer::<T, _>::from_raw(frame.width(), frame.height(), frame.buf_slice())
            .unwrap();

    let rought_scaled = imageops::resize(
        &img,
        thumbnail_width * 2,
        thumbnail_height * 2,
        SCALE_FILTER1,
    );

    imageops::resize(
        &rought_scaled,
        thumbnail_width,
        thumbnail_height,
        SCALE_FILTER2,
    )
    .into_raw()
}
