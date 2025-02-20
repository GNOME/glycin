use std::io::{Cursor, Read};

use glycin_utils::*;
use image::{ImageDecoder, ImageEncoder};
use image_rs::Handler;

pub fn apply(
    mut stream: glycin_utils::UnixStream,
    operations: glycin_utils::operations::Operations,
) -> Result<CompleteEditorOutput, glycin_utils::ProcessError> {
    let mut old_png_data: Vec<u8> = Vec::new();
    stream.read_to_end(&mut old_png_data).internal_error()?;
    let cursor = Cursor::new(&old_png_data);

    let decoder = image::codecs::png::PngDecoder::new(cursor).expected_error()?;

    let mut simple_frame = Handler::default().simple_frame(&decoder).expected_error()?;
    let mut buf = vec![0; decoder.total_bytes() as usize];
    decoder.read_image(&mut buf).expected_error()?;

    buf = operations.apply(buf, &mut simple_frame).expected_error()?;

    let mut new_png_data = Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut new_png_data,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    );

    let width = simple_frame.width;
    let height = simple_frame.height;
    let color_type = image::ExtendedColorType::from(
        simple_frame
            .memory_format
            .to_color_type()
            .internal_error()?,
    );

    encoder
        .write_image(&buf, width, height, color_type)
        .internal_error()?;

    let new_png = gufo::png::Png::new(new_png_data.into_inner()).expected_error()?;
    let mut old_png = gufo::png::Png::new(old_png_data).expected_error()?;

    // Keep old PNG with its metadata but replace image data with the one from new
    // one
    old_png.replace_image_data(&new_png).expected_error()?;

    let data = BinaryData::from_data(&old_png.into_inner()).expected_error()?;

    Ok(CompleteEditorOutput::new(data))
}
