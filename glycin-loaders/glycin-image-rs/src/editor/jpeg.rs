use std::io::Read;

use editing::EditingFrame;
use glycin_utils::*;
use gufo_common::orientation::Orientation;
use gufo_jpeg::Jpeg;
use zune_jpeg::zune_core::options::DecoderOptions;

pub struct EditJpeg {
    buf: Vec<u8>,
}

pub fn load(mut stream: glycin_utils::UnixStream) -> Result<EditJpeg, glycin_utils::ProcessError> {
    let mut buf: Vec<u8> = Vec::new();
    stream.read_to_end(&mut buf).internal_error()?;
    Ok(EditJpeg { buf })
}

pub fn apply_sparse(
    edit_jpeg: &EditJpeg,
    mut operations: Operations,
) -> Result<SparseEditorOutput, glycin_utils::ProcessError> {
    let buf = edit_jpeg.buf.clone();
    let jpeg = gufo::jpeg::Jpeg::new(buf).expected_error()?;

    let metadata = gufo::Metadata::for_jpeg(&jpeg);
    if let Some(orientation) = metadata.orientation() {
        operations.prepend(Operations::new_orientation(orientation));
    }

    if let Some(orientation) = operations.orientation() {
        if let Some(byte_changes) = rotate_sparse(orientation, &jpeg)? {
            return Ok(SparseEditorOutput::byte_changes(byte_changes));
        }
    }

    Ok(SparseEditorOutput::from(apply_non_sparse(
        jpeg, operations,
    )?))
}

pub fn apply_complete(
    edit_jpeg: &EditJpeg,
    mut operations: Operations,
) -> Result<CompleteEditorOutput, glycin_utils::ProcessError> {
    let buf = edit_jpeg.buf.clone();

    let jpeg = gufo::jpeg::Jpeg::new(buf).expected_error()?;

    let metadata = gufo::Metadata::for_jpeg(&jpeg);
    if let Some(orientation) = metadata.orientation() {
        operations.prepend(Operations::new_orientation(orientation));
    }

    if let Some(orientation) = operations.orientation() {
        if let Some(byte_changes) = rotate_sparse(orientation, &jpeg)? {
            let mut data = jpeg.into_inner();
            byte_changes.apply(&mut data);
            return CompleteEditorOutput::new_lossless(data);
        }
    }

    apply_non_sparse(jpeg, operations)
}

fn apply_non_sparse(
    jpeg: Jpeg,
    operations: Operations,
) -> Result<CompleteEditorOutput, glycin_utils::ProcessError> {
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

    pixels = editing::apply_operations(pixels, &mut simple_frame, &operations).expected_error()?;

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

    let remove_metadata_rotate = rotate_sparse(Orientation::Id, &jpeg).ok().flatten();

    let mut out_buf = jpeg.into_inner();

    // Since we apply all operionats, including existing exif orientation, to the
    // image itself, the Exif entry, if it exists, is now wrong
    if let Some(remove_metadata_rotate) = remove_metadata_rotate {
        remove_metadata_rotate.apply(&mut out_buf);
    }

    let binary_data = BinaryData::from_data(out_buf).expected_error()?;
    return Ok(CompleteEditorOutput::new(binary_data));
}

fn rotate_sparse(
    orientation: Orientation,
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

        if let Some(entry) = exif.lookup_entry(gufo_common::field::Orientation) {
            let pos = exif_segment_data_pos
                + entry.value_offset_position() as usize
                + gufo::jpeg::EXIF_IDENTIFIER_STRING.len();

            return Ok(Some(ByteChanges::from_slice(&[(
                pos as u64,
                orientation as u8,
            )])));
        }
    }

    Ok(None)
}
