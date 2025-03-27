use std::io::{Cursor, Read};

use glycin_utils::operations::Operations;
use glycin_utils::*;
use image::{ImageDecoder, ImageEncoder};
use image_rs::Handler;

pub fn apply(
    mut stream: glycin_utils::UnixStream,
    mut operations: glycin_utils::operations::Operations,
) -> Result<CompleteEditorOutput, glycin_utils::ProcessError> {
    let mut old_png_data: Vec<u8> = Vec::new();
    stream.read_to_end(&mut old_png_data).internal_error()?;
    let cursor = Cursor::new(&old_png_data);

    let decoder = image::codecs::png::PngDecoder::new(cursor).expected_error()?;

    let mut simple_frame = Handler::default().simple_frame(&decoder).expected_error()?;
    let mut buf = vec![0; decoder.total_bytes() as usize];
    decoder.read_image(&mut buf).expected_error()?;

    let mut old_png = gufo::png::Png::new(old_png_data).expected_error()?;
    let metadata = gufo::Metadata::for_png(&old_png);
    if let Some(orientation) = metadata.orientation() {
        operations.prepend(Operations::new_orientation(orientation));
    }

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

    // Keep old PNG with its metadata but replace image data with the one from new
    // one
    old_png.replace_image_data(&new_png).expected_error()?;

    let raw_data = reset_exif_orientation(old_png);

    let data = BinaryData::from_data(&raw_data).expected_error()?;

    Ok(CompleteEditorOutput::new(data))
}

fn reset_exif_orientation(mut png: gufo::png::Png) -> Vec<u8> {
    let ornt = png
        .chunks()
        .into_iter()
        .find(|x| x.chunk_type().bytes() == *b"orNT");

    if let Some(ornt) = ornt {
        let _ = gufo::png::remove_chunk!(png, ornt);
    }

    let mut byte_pos = Vec::new();
    for chunk in png.chunks() {
        if matches!(chunk.chunk_type(), gufo::png::ChunkType::eXIf) {
            let exif_data = chunk.chunk_data().to_vec();
            if let Some(tag_position) = exif_orientation_value_position(exif_data) {
                let chunk_position = chunk.unsafe_raw_chunk().complete_data().start as u64;
                byte_pos.push(chunk_position + 8 + tag_position as u64);
            }
        }
    }

    let byte_changes = ByteChanges::from_slice(
        &byte_pos
            .into_iter()
            .map(|x| (x, gufo_common::orientation::Orientation::Id as u8))
            .collect::<Vec<_>>(),
    );

    let mut png_data = png.into_inner();
    byte_changes.apply(&mut png_data);
    png_data
}

fn exif_orientation_value_position(data: Vec<u8>) -> Option<usize> {
    let mut exif = gufo_exif::internal::ExifRaw::new(data);
    exif.decode().ok()?;
    if let Some(entry) = exif.lookup_entry(gufo_common::field::Orientation) {
        Some(entry.value_offset_position() as usize)
    } else {
        None
    }
}
