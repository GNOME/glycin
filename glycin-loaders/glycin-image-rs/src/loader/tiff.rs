use std::io::Cursor;

use glycin_utils::{
    ByteData, Frame, FrameDetails, GenericContexts, ImageDetails, MemoryFormat, ProcessError,
};
use tiff::tags::{
    ExtraSamples::AssociatedAlpha,
    SampleFormat::{IEEEFP, Uint},
};

pub type TiffReader = Cursor<Vec<u8>>;

pub fn load<B: ByteData>(
    data: Vec<u8>,
) -> Result<(ImageDetails<B>, tiff::decoder::Decoder<TiffReader>), ProcessError> {
    let exif = data.clone();
    let mut decoder = tiff::decoder::Decoder::new(Cursor::new(data)).expected_error()?;

    let (width, height) = decoder.dimensions().expected_error()?;

    let mut image_details = ImageDetails::new(width, height);
    image_details.metadata_exif = Some(ByteData::try_from_vec(exif).expected_error()?);
    image_details.metadata_xmp = decoder
        .get_tag_u8_vec(tiff::tags::Tag::Unknown(700))
        .ok()
        .map(B::try_from_vec)
        .transpose()
        .expected_error()?;

    Ok((image_details, decoder))
}

pub fn frame<B: ByteData>(
    decoder: &mut tiff::decoder::Decoder<TiffReader>,
) -> Result<Frame<B>, ProcessError> {
    let color_type = decoder.colortype().expected_error()?;
    let (width, height) = decoder.dimensions().expected_error()?;

    let mut frame_details = FrameDetails::default();

    frame_details.color_icc_profile = decoder
        .get_tag_u8_vec(tiff::tags::Tag::IccProfile)
        .ok()
        .map(B::try_from_vec)
        .transpose()
        .expected_error()?;

    let mut decoding_result = tiff::decoder::DecodingResult::U8(vec![]);
    let layout_preference = decoder
        .read_image_to_buffer(&mut decoding_result)
        .expected_error()?;

    let extra_samples = decoder
        .get_tag_u16_vec(tiff::tags::Tag::ExtraSamples)
        .map(|x| x.first().cloned())
        .ok()
        .flatten()
        .and_then(tiff::tags::ExtraSamples::from_u16)
        .unwrap_or(tiff::tags::ExtraSamples::Unspecified);

    let memory_format = match (color_type, layout_preference.sample_format, extra_samples) {
        (tiff::ColorType::Gray(8), Uint, _) => MemoryFormat::G8,
        (tiff::ColorType::Gray(16), Uint, _) => MemoryFormat::G16,
        (tiff::ColorType::GrayA(8), Uint, AssociatedAlpha) => MemoryFormat::G8a8Premultiplied,
        (tiff::ColorType::GrayA(16), Uint, AssociatedAlpha) => MemoryFormat::G16a16Premultiplied,
        (tiff::ColorType::GrayA(8), Uint, _) => MemoryFormat::G8a8,
        (tiff::ColorType::GrayA(16), Uint, _) => MemoryFormat::G16a16,
        (tiff::ColorType::RGB(8), Uint, _) => MemoryFormat::R8g8b8,
        (tiff::ColorType::RGB(16), Uint, _) => MemoryFormat::R16g16b16,
        (tiff::ColorType::RGB(16), IEEEFP, _) => MemoryFormat::R16g16b16Float,
        (tiff::ColorType::RGB(32), IEEEFP, _) => MemoryFormat::R32g32b32Float,
        (tiff::ColorType::RGBA(8), Uint, AssociatedAlpha) => MemoryFormat::R8g8b8a8Premultiplied,
        (tiff::ColorType::RGBA(16), Uint, AssociatedAlpha) => {
            MemoryFormat::R16g16b16a16Premultiplied
        }
        (tiff::ColorType::RGBA(32), IEEEFP, AssociatedAlpha) => {
            MemoryFormat::R32g32b32a32FloatPremultiplied
        }
        (tiff::ColorType::RGBA(8), Uint, _) => MemoryFormat::R8g8b8a8,
        (tiff::ColorType::RGBA(16), Uint, _) => MemoryFormat::R16g16b16a16,
        (tiff::ColorType::RGBA(16), IEEEFP, _) => MemoryFormat::R16g16b16a16Float,
        (tiff::ColorType::RGBA(32), IEEEFP, _) => MemoryFormat::R32g32b32a32Float,
        (tiff::ColorType::CMYK(8), Uint, _) => MemoryFormat::C8m8y8k8,
        (tiff::ColorType::CMYKA(8), Uint, _) => MemoryFormat::C8m8y8k8a8,
        (color_type, sample_format, extra_samples) => {
            return Err(ProcessError::expected(&format!(
                "ColorType {color_type:?} with SampleFormat {sample_format:?} and ExtraSamples {extra_samples:?} not supported"
            )));
        }
    };

    let texture =
        ByteData::try_from_slice(decoding_result.as_buffer(0).as_bytes()).expected_error()?;

    let mut frame = Frame::new(width, height, memory_format, texture).expected_error()?;

    frame.details = frame_details;

    Ok(frame)
}
