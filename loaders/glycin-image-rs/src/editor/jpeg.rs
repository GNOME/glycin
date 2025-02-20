use std::io::Read;

use editing::EditingFrame;
use glycin_utils::*;
use gufo_common::orientation::Rotation;
use gufo_jpeg::Jpeg;
use memory_format::ExtendedMemoryFormat;
use operations::Operation;
use zune_jpeg::zune_core::options::DecoderOptions;

pub fn apply_sparse(
    mut stream: glycin_utils::UnixStream,
    operations: glycin_utils::operations::Operations,
) -> Result<SparseEditorOutput, glycin_utils::ProcessError> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).internal_error()?;
    let jpeg = gufo::jpeg::Jpeg::new(buf).expected_error()?;

    if operations.operations().len() == 1 {
        if let Operation::Rotate(rotation) = operations.operations().first().expected_error()? {
            if let Some(byte_changes) = rotate_sparse(rotation, &jpeg)? {
                return Ok(SparseEditorOutput::byte_changes(byte_changes));
            }
        }
    }

    Ok(SparseEditorOutput::from(apply_complete(
        stream, operations,
    )?))
}

pub fn apply_complete(
    mut stream: glycin_utils::UnixStream,
    operations: glycin_utils::operations::Operations,
) -> Result<CompleteEditorOutput, glycin_utils::ProcessError> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).internal_error()?;
    let jpeg = gufo::jpeg::Jpeg::new(buf).expected_error()?;

    if operations.operations().len() == 1 {
        if let Operation::Rotate(rotation) = operations.operations().first().expected_error()? {
            if let Some(byte_changes) = rotate_sparse(rotation, &jpeg)? {
                let mut data = jpeg.into_inner();
                byte_changes.apply(&mut data);
                return CompleteEditorOutput::new_lossless(data);
            }
        }
    }

    let mut out_buf = Vec::new();
    let encoder = jpeg.encoder(&mut out_buf).expected_error()?;
    let buf = jpeg.into_inner();

    let decoder_options = DecoderOptions::new_fast()
        .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::YCbCr)
        .set_max_height(u32::MAX as usize)
        .set_max_width(u32::MAX as usize);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(&buf, decoder_options);
    let mut pixels = decoder.decode().expected_error()?;
    let info: zune_jpeg::ImageInfo = decoder.info().expected_error()?;
    let mut simple_frame = EditingFrame {
        width: info.width as u32,
        height: info.height as u32,
        stride: info.width as u32 * 3,
        memory_format: ExtendedMemoryFormat::Y8Cb8Cr8,
    };

    pixels = operations
        .apply(pixels, &mut simple_frame)
        .expected_error()?;

    encoder
        .encode(
            &pixels,
            simple_frame.width as u16,
            simple_frame.height as u16,
            jpeg_encoder::ColorType::Ycbcr,
        )
        .expected_error()?;

    let mut jpeg = gufo::jpeg::Jpeg::new(buf).expected_error()?;
    let new_jpeg = Jpeg::new(out_buf).expected_error()?;

    jpeg.replace_image_data(&new_jpeg).expected_error()?;

    let out_buf = jpeg.into_inner();

    let binary_data = BinaryData::from_data(out_buf)?;
    return Ok(CompleteEditorOutput::new(binary_data));
}

fn rotate_sparse(
    rotation: &Rotation,
    jpeg: &Jpeg,
) -> Result<Option<ByteChanges>, glycin_utils::ProcessError> {
    let exif_data = jpeg.exif_data().map(|x| x.to_vec()).collect::<Vec<_>>();
    let mut exif_data = exif_data.into_iter();
    let exif_segment = jpeg
        .exif_segments()
        .map(|x| x.data_pos())
        .collect::<Vec<_>>();
    let mut exif_segment = exif_segment.iter();

    if let (Some(exif_data), Some(exif_segment_data_pos)) = (exif_data.next(), exif_segment.next())
    {
        let mut exif = gufo_exif::internal::ExifRaw::new(exif_data.to_vec());
        exif.decode().expected_error()?;

        if let (Some(entry), Some(current_orientation)) = (
            exif.lookup_entry(gufo_common::field::Orientation),
            exif.lookup_short(gufo_common::field::Orientation)
                .expected_error()?,
        ) {
            let pos = exif_segment_data_pos
                + entry.value_offset_position() as usize
                + gufo::jpeg::EXIF_IDENTIFIER_STRING.len();

            let current_orientation =
                gufo_common::orientation::Orientation::try_from(current_orientation)
                    .expected_error()?;

            let new_rotation = current_orientation.rotate() - *rotation;

            let new_orientation = gufo_common::orientation::Orientation::new(
                new_rotation,
                current_orientation.mirror(),
            );

            return Ok(Some(ByteChanges::from_slice(&[(
                pos as u64,
                new_orientation as u8,
            )])));
        }
    }

    Ok(None)
}
