use std::io::{Cursor, Read};

use glycin_utils::{image_rs, *};
use gufo::png::NewChunk;
use gufo_common::error::ErrorWithData;
use gufo_common::physical_dimension::PhysicalDimensionUnit;
use gufo_common::{field, orientation};
use gufo_exif::Exif;
use image::{ExtendedColorType, ImageEncoder};

pub struct EditorPng {
    png: gufo::png::Png,
    metadata: gufo::Metadata,
    editing_frame: glycin_utils::editing::EditingFrame<LocalMemory>,
}

pub fn create<B: ByteData>(
    new_image: NewImage<B>,
    frame: Frame<FungibleMemory>,
    encoding_options: EncodingOptions,
    memory_format: ExtendedColorType,
    icc_profile: Option<Vec<u8>>,
) -> Result<Vec<u8>, ProcessError> {
    let compression = if let Some(compression) = encoding_options.compression {
        if compression < 30 {
            image::codecs::png::CompressionType::Fast
        } else if compression < 80 {
            image::codecs::png::CompressionType::Default
        } else {
            image::codecs::png::CompressionType::Best
        }
    } else {
        image::codecs::png::CompressionType::Default
    };

    let mut out_buf = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut out_buf,
        compression,
        image::codecs::png::FilterType::default(),
    );

    if let Some(icc_profile) = icc_profile {
        let _ = encoder.set_icc_profile(icc_profile);
    }

    encoder
        .write_image(&frame.texture, frame.width, frame.height, memory_format)
        .internal_error()?;

    Ok(add_metadata(out_buf, &new_image.image_info, &frame.details))
}

pub fn load<S: Read>(mut stream: S) -> Result<EditorPng, glycin_utils::ProcessError> {
    let mut old_png_data: Vec<u8> = Vec::new();
    stream.read_to_end(&mut old_png_data).internal_error()?;
    let cursor = Cursor::new(&old_png_data);

    let decoder = image::codecs::png::PngDecoder::new(cursor).expected_error()?;

    let editing_frame = image_rs::Handler::default()
        .editing_frame(decoder)
        .expected_error()?;

    let png: gufo::png::Png = gufo::png::Png::new(old_png_data).expected_error()?;
    let metadata = gufo::Metadata::for_png(&png);

    Ok(EditorPng {
        png,
        metadata,
        editing_frame,
    })
}

pub fn apply<B: ByteData>(
    img_editor: &EditorPng,
    mut operations: Operations,
) -> Result<CompleteEditorOutput<B>, glycin_utils::ProcessError> {
    if let Some(orientation) = img_editor.metadata.orientation() {
        operations.prepend(Operations::new_orientation(orientation));
    }

    let editing_frame = img_editor.editing_frame.clone();
    let mut old_png = img_editor.png.clone();

    let editing_frame =
        editing::apply_operations(editing_frame.into_funglible(), &operations).expected_error()?;

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
    let texture = editing_frame.texture;

    encoder
        .write_image(&texture, width, height, color_type)
        .internal_error()?;

    let new_png = gufo::png::Png::new(new_png_data.into_inner()).expected_error()?;

    // Keep old PNG with its metadata but replace image data with the one from new
    // one
    old_png.replace_image_data(&new_png).expected_error()?;

    let raw_data = reset_exif_orientation(old_png)?;

    let data = B::try_from_vec(raw_data).expected_error()?;

    Ok(CompleteEditorOutput::new(data))
}

fn reset_exif_orientation(mut png: gufo::png::Png) -> Result<Vec<u8>, glycin_utils::ProcessError> {
    let ornt = png
        .chunks()
        .into_iter()
        .find(|x| x.chunk_type().bytes() == *b"orNT");

    if let Some(ornt) = ornt {
        let _ = gufo::png::remove_chunk!(png, ornt);
    }

    let mut byte_updates = Vec::new();

    let mut chunks = png.chunks().into_iter();

    while let Some(chunk) = chunks.next() {
        if matches!(chunk.chunk_type(), gufo::png::ChunkType::eXIf) {
            let exif_data = chunk.chunk_data().to_vec();
            if let Some(tag_position) = exif_orientation_value_position(exif_data) {
                let chunk_position = chunk.unsafe_raw_chunk().complete_data().start as u64;
                for (pos, value) in tag_position {
                    byte_updates.push((pos as u64 + chunk_position + 8, value));
                }
            }
        } else if let Some(mut exif_data) = chunk.legacy_exif(100 * 1000 * 1000) {
            // This chunk is compressed, so we have to rewrite it

            match Exif::for_mut_slice(&mut exif_data) {
                Err(err) => {
                    log::info!("Exif decode failed: {err}");
                }
                Ok(mut exif) => {
                    if let Some(orientation_entry) = exif.orientation()
                        && orientation_entry != orientation::Orientation::Id
                    {
                        if let Err(err) = exif.update_entry_diff(
                            field::Orientation.into(),
                            gufo_exif::Typed::Short(vec![orientation::Orientation::Id as u16]),
                        ) {
                            log::info!("Failed to update Exif orientation tag {err}");
                        }

                        if let Err(err) = gufo::png::remove_chunk!(png, chunk) {
                            log::info!("Failed to remove chunk: {err}");
                        }
                        let new_chunk =
                            gufo::png::NewChunk::new(gufo::png::ChunkType::eXIf, exif_data);
                        if let Err(err) = png.insert_chunk(new_chunk) {
                            log::info!("Failed to insert eXIf chunk: {err}");
                        }
                        break;
                    }
                }
            }
        }
    }

    let byte_changes = ByteChanges::from_slice(&byte_updates);

    let mut png_data = png.into_inner();
    byte_changes.apply(&mut png_data).internal_error()?;
    Ok(png_data)
}

fn exif_orientation_value_position(data: Vec<u8>) -> Option<Vec<(usize, u8)>> {
    let mut exif = gufo_exif::Exif::for_vec(data).ok()?;
    exif.update_entry_diff(
        field::Orientation.into(),
        gufo_exif::Typed::Short(vec![orientation::Orientation::Id as u16]),
    )
    .ok()
}

pub fn add_metadata<B: ByteData, C: ByteData>(
    buf: Vec<u8>,
    image_info: &ImageDetails<B>,
    frame_details: &FrameDetails<C>,
) -> Vec<u8> {
    match add_metadata_internal(buf, image_info, frame_details) {
        Err(err) => {
            log::error!("Failed to add metadata: {err}");
            err.into_inner()
        }
        Ok(buf) => buf,
    }
}

fn add_metadata_internal<B: ByteData, C: ByteData>(
    buf: Vec<u8>,
    image_info: &ImageDetails<B>,
    frame_details: &FrameDetails<C>,
) -> Result<Vec<u8>, ErrorWithData<gufo::png::Error>> {
    let mut png = gufo::png::Png::new(buf)?;

    let mut new_chunks = Vec::new();

    if let Some(key_value) = &image_info.metadata_key_value {
        for (key, value) in key_value {
            new_chunks.push(NewChunk::text(key, value));
        }
    }

    if let Some(pixel_density) = &frame_details.pixel_density {
        let pixel_density = pixel_density.convert(PhysicalDimensionUnit::Meter);
        new_chunks.push(NewChunk::phys_meter(
            pixel_density.x().value().round() as u32,
            pixel_density.y().value().round() as u32,
        ));
    }

    for chunk in new_chunks {
        if let Err(err) = png.insert_chunk(chunk) {
            return Err(ErrorWithData::new(err, png.into_inner()));
        }
    }

    Ok(png.into_inner())
}
