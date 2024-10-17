use std::io::{Cursor, Read};

use glycin_utils::*;
use image::{ImageDecoder, ImageEncoder};
use image_rs::Handler;
use operations::Operation;

#[derive(Default)]
pub struct ImgEditor {}

impl EditorImplementation for ImgEditor {
    fn apply_sparse(
        &self,
        stream: glycin_utils::UnixStream,
        mime_type: String,
        details: glycin_utils::InitializationDetails,
        operations: glycin_utils::operations::Operations,
    ) -> Result<SparseEditorOutput, glycin_utils::ProcessError> {
        match mime_type.as_str() {
            "image/jpeg" => apply_jpeg(stream, operations),
            _ => Ok(SparseEditorOutput::from_complete(
                self.apply_complete(stream, mime_type, details, operations)?,
            )),
        }
    }

    fn apply_complete(
        &self,
        stream: UnixStream,
        mime_type: String,
        _details: InitializationDetails,
        operations: operations::Operations,
    ) -> Result<CompleteEditorOutput, ProcessError> {
        match mime_type.as_str() {
            "image/png" => apply_png(stream, operations),
            mime_type => Err(ProcessError::UnsupportedImageFormat(mime_type.to_string())),
        }
    }
}

fn apply_png(
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

    let new_png = gufo_png::Png::new(new_png_data.into_inner()).expected_error()?;
    let mut old_png = gufo_png::Png::new(old_png_data).expected_error()?;

    // Keep old PNG with its metadata but replace image data with the one from new
    // one
    old_png.replace_image_data(&new_png).expected_error()?;

    let data = BinaryData::from_data(&old_png.into_inner()).expected_error()?;

    Ok(CompleteEditorOutput::new(data))
}

fn apply_jpeg(
    mut stream: glycin_utils::UnixStream,
    operations: glycin_utils::operations::Operations,
) -> Result<SparseEditorOutput, glycin_utils::ProcessError> {
    match operations.operations().first().expected_error()? {
        Operation::Rotate(rotation) => {
            let mut buf = Vec::new();
            stream.read_to_end(&mut buf).internal_error()?;

            let jpeg = gufo_jpeg::Jpeg::new(buf).unwrap();
            let exif_data = jpeg.exif_data().map(|x| x.to_vec()).collect::<Vec<_>>();
            let mut exif_data = exif_data.into_iter();
            let exif_segment = jpeg.exif().map(|x| x.data_pos()).collect::<Vec<_>>();
            let mut exif_segment = exif_segment.iter();
            let buf = jpeg.into_inner();

            if let (Some(exif_data), Some(exif_segment_data_pos)) =
                (exif_data.next(), exif_segment.next())
            {
                let mut exif = gufo_exif::internal::ExifRaw::new(exif_data.to_vec());
                exif.decode().expected_error()?;

                if let Some(entry) = exif.lookup_entry(gufo_common::field::Orientation) {
                    let pos = exif_segment_data_pos
                        + entry.value_offset_position() as usize
                        + gufo_jpeg::EXIF_IDENTIFIER_STRING.len();

                    let current_orientation =
                        gufo_common::orientation::Orientation::try_from(buf[pos] as u16)
                            .expected_error()?;

                    let new_rotation = current_orientation.rotate() - *rotation;

                    let new_orientation = gufo_common::orientation::Orientation::new(
                        new_rotation,
                        current_orientation.mirror(),
                    );

                    return Ok(SparseEditorOutput::bit_changes(&[(
                        pos as u64,
                        new_orientation as u8,
                    )]));
                }
            }
        }
        _ => {
            todo!()
        }
    }

    // TODO: This should probably be an error?
    Ok(SparseEditorOutput::bit_changes(&[]))
}
