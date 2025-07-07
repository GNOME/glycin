use std::io::{Cursor, Read};
use std::sync::Arc;

use glycin_utils::{image_rs, *};
use gufo::png::NewChunk;
use gufo_common::error::ErrorWithData;
use gufo_exif::internal::ExifRaw;
use image::{ImageDecoder, ImageEncoder};

pub struct EditorPng {
    png: gufo::png::Png,
    metadata: gufo::Metadata,
    frame_buf: Vec<u8>,
    editing_frame: glycin_utils::editing::EditingFrame,
}

pub fn load(mut stream: glycin_utils::UnixStream) -> Result<EditorPng, glycin_utils::ProcessError> {
    let mut old_png_data: Vec<u8> = Vec::new();
    stream.read_to_end(&mut old_png_data).internal_error()?;
    let cursor = Cursor::new(&old_png_data);

    let decoder = image::codecs::png::PngDecoder::new(cursor).expected_error()?;

    let editing_frame = image_rs::Handler::default()
        .editing_frame(&decoder)
        .expected_error()?;
    let mut frame_buf = vec![0; decoder.total_bytes() as usize];
    decoder.read_image(&mut frame_buf).expected_error()?;

    let png: gufo::png::Png = gufo::png::Png::new(old_png_data).expected_error()?;
    let metadata = gufo::Metadata::for_png(&png);

    Ok(EditorPng {
        png,
        metadata,
        frame_buf,
        editing_frame,
    })
}

pub fn apply(
    img_editor: &EditorPng,
    mut operations: Operations,
) -> Result<CompleteEditorOutput, glycin_utils::ProcessError> {
    if let Some(orientation) = img_editor.metadata.orientation() {
        operations.prepend(Operations::new_orientation(orientation));
    }

    let mut editing_frame = img_editor.editing_frame.clone();
    let mut buf = img_editor.frame_buf.clone();
    let mut old_png = img_editor.png.clone();

    buf = editing::apply_operations(buf, &mut editing_frame, &operations).expected_error()?;

    let mut new_png_data = Cursor::new(Vec::new());
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut new_png_data,
        image::codecs::png::CompressionType::Default,
        image::codecs::png::FilterType::Adaptive,
    );

    let width = editing_frame.width;
    let height = editing_frame.height;
    let color_type = image::ExtendedColorType::from(
        image_rs::extended_memory_format_to_color_type(&editing_frame.memory_format)
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

    let mut chunks = png.chunks().into_iter();

    while let Some(chunk) = chunks.next() {
        if matches!(chunk.chunk_type(), gufo::png::ChunkType::eXIf) {
            let exif_data = chunk.chunk_data().to_vec();
            if let Some(tag_position) = exif_orientation_value_position(exif_data) {
                let chunk_position = chunk.unsafe_raw_chunk().complete_data().start as u64;
                byte_pos.push(chunk_position + 8 + tag_position as u64);
            }
        } else if let Some(exif_data) = chunk.legacy_exif(100 * 1000 * 1000) {
            let mut exif = ExifRaw::new(exif_data);
            if let Err(err) = exif.decode() {
                log::info!("Exif decode failed: {err}");
            }

            if let Some(orientation_entry) = exif.lookup_entry(gufo_common::field::Orientation) {
                if orientation_entry.u32() != Some(gufo_common::orientation::Orientation::Id as u32)
                {
                    if let Err(err) = exif.set_existing(
                        gufo_common::field::Orientation,
                        gufo_common::orientation::Orientation::Id as u32,
                    ) {
                        log::info!("Failed to update Exif orientation tag {err}");
                    }
                    if let Some(exif_data) =
                        Arc::into_inner(exif.raw.buffer).and_then(|x| x.into_inner().ok())
                    {
                        drop(chunks);
                        if let Err(err) = gufo::png::remove_chunk!(png, chunk) {
                            log::info!("Failed to remove chunk: {err}");
                        }
                        let new_chunk = gufo::png::NewChunk::new(
                            gufo::png::ChunkType::eXIf,
                            exif_data.into_inner(),
                        );
                        if let Err(err) = png.insert_chunk(new_chunk) {
                            log::info!("Failed to insert eXIf chunk: {err}");
                        }
                        break;
                    }
                }
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

pub fn add_metadata(
    buf: Vec<u8>,
    image_info: &ImageDetails,
    frame_details: &FrameDetails,
) -> Vec<u8> {
    match add_metadata_internal(buf, image_info, frame_details) {
        Err(err) => {
            log::error!("Failed to add metadata: {err}");
            err.into_inner()
        }
        Ok(buf) => buf,
    }
}

fn add_metadata_internal(
    buf: Vec<u8>,
    image_info: &ImageDetails,
    _frame_details: &FrameDetails,
) -> Result<Vec<u8>, ErrorWithData<gufo::png::Error>> {
    let mut png = gufo::png::Png::new(buf)?;

    if let Some(key_value) = &image_info.metadata_key_value {
        for (key, value) in key_value {
            if let Err(err) = png.insert_chunk(NewChunk::text(key, value)) {
                return Err(ErrorWithData::new(err, png.into_inner()));
            }
        }
    }

    Ok(png.into_inner())
}
