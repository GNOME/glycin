use std::io::{Cursor, Read};

use glycin_utils::{image_rs, *};
use gufo_common::physical_dimension::PhysicalDimensionUnit;
use gufo_common::{field, orientation};
use gufo_exif::Exif;
use image::ImageEncoder;

pub struct EditorPng {
    png: gufo::png::Png,
    metadata: gufo::Metadata,
    editing_frame: glycin_utils::editing::EditingFrame<LocalMemory>,
}

pub fn create<B: ByteData>(
    new_image: NewImage<B>,
    frame: Frame<FungibleMemory>,
    encoding_options: EncodingOptions,
) -> Result<Vec<u8>, ProcessError> {
    let compression = if let Some(compression) = encoding_options.compression {
        if compression < 20 {
            png::Compression::NoCompression
        } else if compression < 40 {
            png::Compression::Fastest
        } else if compression < 60 {
            png::Compression::Fast
        } else if compression < 80 {
            png::Compression::Balanced
        } else {
            png::Compression::High
        }
    } else {
        png::Compression::Balanced
    };

    let mut out_buf = Vec::new();

    let (color_type, bit_depth) = match frame.memory_format {
        MemoryFormat::G8 => (png::ColorType::Grayscale, png::BitDepth::Eight),
        MemoryFormat::G8a8 => (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight),
        MemoryFormat::R8g8b8 => (png::ColorType::Rgb, png::BitDepth::Eight),
        MemoryFormat::R8g8b8a8 => (png::ColorType::Rgba, png::BitDepth::Eight),
        MemoryFormat::G16 => (png::ColorType::Grayscale, png::BitDepth::Sixteen),
        MemoryFormat::G16a16 => (png::ColorType::GrayscaleAlpha, png::BitDepth::Sixteen),
        MemoryFormat::R16g16b16 => (png::ColorType::Rgb, png::BitDepth::Sixteen),
        MemoryFormat::R16g16b16a16 => (png::ColorType::Rgba, png::BitDepth::Sixteen),
        _ => panic!(),
    };

    let mut info = png::Info::with_size(frame.width, frame.height);
    info.icc_profile = frame
        .details
        .color_icc_profile
        .as_deref()
        .map(std::borrow::Cow::Borrowed);
    info.color_type = color_type;
    info.bit_depth = bit_depth;
    info.interlaced = frame.details.encoding_progressive.unwrap_or_default();
    info.exif_metadata = new_image
        .image_info
        .metadata_exif
        .as_deref()
        .map(|x| std::borrow::Cow::Borrowed(x));

    if let Some(cicp) = frame.details.color_cicp {
        info.coding_independent_code_points = Some(png::CodingIndependentCodePoints {
            color_primaries: cicp[0],
            transfer_function: cicp[1],
            matrix_coefficients: cicp[2],
            is_video_full_range_image: cicp[3] != 0,
        });
    }

    if let Some(pixel_density) = frame.details.pixel_density {
        let pixel_density = pixel_density.convert(PhysicalDimensionUnit::Meter);

        info.pixel_dims = Some(png::PixelDimensions {
            unit: png::Unit::Meter,
            xppu: pixel_density.x().value().round() as u32,
            yppu: pixel_density.y().value().round() as u32,
        });
    }

    if let Some(key_value) = new_image.image_info.metadata_key_value {
        info.uncompressed_latin1_text = key_value
            .iter()
            .map(|(k, v)| png::text_metadata::TEXtChunk::new(k, v))
            .collect();
    }

    let mut encoder = png::Encoder::with_info(&mut out_buf, info).expected_error()?;

    encoder.set_compression(compression);

    let mut writer = encoder.write_header().expected_error()?;
    writer.write_image_data(&frame.texture).expected_error()?;
    drop(writer);

    Ok(out_buf)
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

    let chunks = png.chunks().into_iter();

    for chunk in chunks {
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
