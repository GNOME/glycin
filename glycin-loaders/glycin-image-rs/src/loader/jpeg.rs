use std::io::Cursor;

use glycin_utils::{
    ByteData, Frame, FrameDetails, GenericContexts, ImageDetails, MemoryFormat, ProcessError,
};
use zune_jpeg::zune_core;
use zune_jpeg::zune_core::colorspace;

pub type JpegReader = Cursor<Vec<u8>>;

pub fn load<B: ByteData>(
    data: Vec<u8>,
) -> Result<(ImageDetails<B>, zune_jpeg::JpegDecoder<JpegReader>), ProcessError> {
    let mut decoder = zune_jpeg::JpegDecoder::new(Cursor::new(data));

    decoder.decode_headers().expected_error()?;

    let colorspace = match decoder.input_colorspace().internal_error()? {
        c @ (colorspace::ColorSpace::Luma
        | colorspace::ColorSpace::RGB
        | colorspace::ColorSpace::CMYK
        | colorspace::ColorSpace::YCCK) => c,
        _ => colorspace::ColorSpace::RGB,
    };

    let options = zune_core::options::DecoderOptions::new_fast()
        .jpeg_set_out_colorspace(colorspace)
        .set_max_width(usize::MAX)
        .set_max_height(usize::MAX);
    decoder.set_options(options);

    let (width, height) = decoder.dimensions().internal_error()?;
    let mut image_details = ImageDetails::new(width as u32, height as u32);
    image_details.metadata_exif = decoder
        .exif()
        .map(|x| B::try_from_slice(x))
        .transpose()
        .unwrap();
    image_details.metadata_xmp = decoder
        .xmp()
        .map(|x| B::try_from_slice(x))
        .transpose()
        .unwrap();

    Ok((image_details, decoder))
}

pub fn frame<B: ByteData>(
    decoder: &mut zune_jpeg::JpegDecoder<JpegReader>,
) -> Result<Frame<B>, ProcessError> {
    let (width, height) = decoder.dimensions().unwrap();
    let color = decoder.output_colorspace().unwrap();
    let n_components = color.num_components();
    let mut texture = B::new((width * height * n_components) as u64).expected_error()?;
    decoder.decode_into(&mut texture).expected_error()?;

    let memory_format = match color {
        colorspace::ColorSpace::RGB => MemoryFormat::R8g8b8,
        colorspace::ColorSpace::Luma => MemoryFormat::G8,
        colorspace::ColorSpace::CMYK => {
            jpeg_cmyk_to_cmyk(&mut texture);
            MemoryFormat::C8m8y8k8
        }
        colorspace::ColorSpace::YCCK => {
            jpeg_ycck_to_cmyk(&mut texture);
            MemoryFormat::C8m8y8k8
        }
        c => unreachable!("{c:?}"),
    };

    let mut frame_details = FrameDetails::default();
    frame_details.color_icc_profile = decoder
        .icc_profile()
        .map(B::try_from_vec)
        .transpose()
        .expected_error()?;

    let mut frame =
        Frame::new(width as u32, height as u32, memory_format, texture).expected_error()?;
    frame.details = frame_details;

    Ok(frame)
}

/// Naive Rec. T.871 implementation
fn jpeg_ycck_to_cmyk(data: &mut [u8]) {
    for chunk in data.as_chunks_mut::<4>().0 {
        let y = 255. - chunk[0] as f32;
        let cb = 255. - chunk[1] as f32;
        let cr = 255. - chunk[2] as f32;

        let r = y + 1.402 * (cr - 128.);
        let g = y - 0.3441 * (cb - 128.) - 0.7141 * (cr - 128.);
        let b = y + 1.772 * (cb - 128.);

        chunk[0] = (255. - r).round() as u8;
        chunk[1] = (255. - g).round() as u8;
        chunk[2] = (255. - b).round() as u8;
        chunk[3] = 255 - chunk[3];
    }
}

fn jpeg_cmyk_to_cmyk(data: &mut [u8]) {
    for channel in data {
        *channel = 255 - *channel;
    }
}
