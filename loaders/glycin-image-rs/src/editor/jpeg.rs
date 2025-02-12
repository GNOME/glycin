use std::io::Read;

use editing::EditingFrame;
use glycin_utils::*;
use gufo_jpeg::Jpeg;
use memory_format::ExtendedMemoryFormat;
use operations::Operation;
use zune_jpeg::zune_core::options::DecoderOptions;

pub fn apply(
    mut stream: glycin_utils::UnixStream,
    operations: glycin_utils::operations::Operations,
) -> Result<EditorOuput, glycin_utils::ProcessError> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).internal_error()?;
    let jpeg = gufo::jpeg::Jpeg::new(buf).expected_error()?;
    if operations.operations().len() > 1
        || !matches!(operations.operations().first(), Some(Operation::Rotate(_)))
    {
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
        return Ok(EditorOuput::complete(binary_data));
    }

    match operations.operations().first().expected_error()? {
        Operation::Rotate(rotation) => {
            let exif_data = jpeg.exif_data().map(|x| x.to_vec()).collect::<Vec<_>>();
            let mut exif_data = exif_data.into_iter();
            let exif_segment = jpeg
                .exif_segments()
                .map(|x| x.data_pos())
                .collect::<Vec<_>>();
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
                        + gufo::jpeg::EXIF_IDENTIFIER_STRING.len();

                    let current_orientation =
                        gufo_common::orientation::Orientation::try_from(buf[pos] as u16)
                            .expected_error()?;

                    let new_rotation = current_orientation.rotate() - *rotation;

                    let new_orientation = gufo_common::orientation::Orientation::new(
                        new_rotation,
                        current_orientation.mirror(),
                    );

                    return Ok(EditorOuput::sparse(&[(pos as u64, new_orientation as u8)]));
                }
            }
        }
        _ => {
            todo!()
        }
    }

    // TODO: This should probably be an error?
    Ok(EditorOuput::sparse(&[]))
}
