use glycin_utils::*;
use gufo_common::cicp::{Cicp, ColorPrimaries, MatrixCoefficients, TransferCharacteristics};
use jxl::api::states::WithImageInfo;
use jxl::api::{
    Endianness, JxlAuxBoxType, JxlBitDepth, JxlColorEncoding, JxlColorProfile, JxlColorType,
    JxlDataFormat, JxlDecoder, JxlDecoderOptions, JxlExtraChannel, JxlOutputBuffer, JxlPixelFormat,
    JxlPrimaries, JxlTransferFunction, JxlWhitePoint, ProcessingResult, VisibleFrameInfo,
};
use jxl::headers::extra_channels::ExtraChannel;

use jxl::api::{JxlParallelRunner, JxlParallelRunnerFun};
use rayon::prelude::*;

pub struct JxlLoader {
    decoder: Option<JxlDecoder<WithImageInfo>>,
    scanned_frames: Vec<VisibleFrameInfo>,
    num_loops: usize,
    current_loop_idx: usize,
    next_frame: usize,
    data: Vec<u8>,
    offset: usize,
    memory_format: MemoryFormat,
    icc_profile: Option<Vec<u8>>,
    cicp: Option<Cicp>,
}

pub fn load<B: ByteData>(data: Vec<u8>) -> Result<(JxlLoader, ImageDetails<B>), ProcessError> {
    let mut data_slice = data.as_slice();
    let mut options = JxlDecoderOptions::default();
    options.scan_frames_only = true;
    options.request_aux_boxes = vec![JxlAuxBoxType::EXIF];
    let decoder = JxlDecoder::new(options);

    let result = decoder.process(&mut data_slice, None).expected_error()?;
    let mut decoder = match result {
        ProcessingResult::Complete { result } => result,
        ProcessingResult::NeedsMoreInput { .. } => {
            return Err(ProcessError::expected(&"unexpected end of file"));
        }
    };

    let (xsize, ysize) = decoder.basic_info().size;
    let xsize = u32::try_from(xsize).expected_error()?;
    let ysize = u32::try_from(ysize).expected_error()?;

    let mut image_info = ImageDetails::new(xsize, ysize);
    image_info.info_format_name = Some(String::from("JPEG XL"));
    image_info.transformation_ignore_exif = true;

    // Perform a quick frame scan.
    while decoder.has_more_frames() {
        let frame_decoder = match decoder.process(&mut data_slice, None).expected_error()? {
            ProcessingResult::Complete { result } => result,
            ProcessingResult::NeedsMoreInput { .. } => {
                return Err(ProcessError::expected(&"unexpected end of file"));
            }
        };

        decoder = match frame_decoder.skip_frame(&mut data_slice).expected_error()? {
            ProcessingResult::Complete { result } => result,
            ProcessingResult::NeedsMoreInput { .. } => {
                return Err(ProcessError::expected(&"unexpected end of file"));
            }
        };
    }

    // Process trailing data for complete metadata.
    let decoder = match decoder
        .process_trailing_data(&mut data_slice)
        .expected_error()?
    {
        ProcessingResult::Complete { result } => result,
        ProcessingResult::NeedsMoreInput { .. } => {
            return Err(ProcessError::expected(&"unexpected end of file"));
        }
    };

    // There can be multiple Exif boxes in a JPEG XL container, however exact meaning of those
    // boxes in that case is not defined. We can set only one Exif metadata to an image anyway,
    // so take the first Exif box in a container.
    let trailing_box_exif = decoder
        .trailing_box()
        .filter(|b| b.box_type() == JxlAuxBoxType::EXIF);
    let exif_buf = decoder
        .aux_boxes(JxlAuxBoxType::EXIF)
        .first()
        .map(|b| b.data(&[]))
        .or_else(|| trailing_box_exif.map(|b| b.data(data_slice)))
        .transpose()
        .expected_error()?;
    if let Some(exif_buf) = exif_buf {
        // The buffer needs to be processed for actual Exif metadata. There is no documentation
        // about this in jxl-rs yet; See
        // https://libjxl.readthedocs.io/en/stable/api_decoder.html#_CPPv420JxlDecoderGetBoxTypeP10JxlDecoder10JxlBoxType8JXL_BOOL
        // for how to handle each box type.
        let offset = u32::from_be_bytes(*exif_buf.first_chunk::<4>().expected_error()?);
        let offset = offset as usize + 4;
        image_info.metadata_exif =
            Some(B::try_from_slice(&exif_buf.get(offset..).expected_error()?).expected_error()?);
    }

    let scanned_frames = decoder.scanned_frames().to_vec();
    drop(decoder);

    // Prepare another decoder for actual rendering.
    let decoder = JxlDecoder::new(JxlDecoderOptions::default());
    let mut data_slice = &data[..];

    let result = decoder.process(&mut data_slice, None).expected_error()?;
    let mut decoder = match result {
        ProcessingResult::Complete { result } => result,
        ProcessingResult::NeedsMoreInput { .. } => {
            return Err(ProcessError::expected(&"unexpected end of file"));
        }
    };

    let is_grayscale = decoder.current_pixel_format().color_type.is_grayscale();
    let alpha = decoder
        .basic_info()
        .extra_channels
        .iter()
        .find(|ec| ec.ec_type == ExtraChannel::Alpha);
    let bit_depth = &decoder.basic_info().bit_depth;
    let memory_format = memory_format(is_grayscale, bit_depth, alpha);

    // Remap from computed memory format.
    let color_type = match memory_format.n_channels() {
        1 => JxlColorType::Grayscale,
        2 => JxlColorType::GrayscaleAlpha,
        3 => JxlColorType::Rgb,
        4 => JxlColorType::Rgba,
        _ => unreachable!(),
    };
    let endianness = Endianness::native();
    let color_data_format = match memory_format.channel_type() {
        glycin_common::ChannelType::U8 => JxlDataFormat::U8 { bit_depth: 8 },
        glycin_common::ChannelType::U16 => JxlDataFormat::U16 {
            bit_depth: 16,
            endianness,
        },
        glycin_common::ChannelType::F16 => JxlDataFormat::F16 { endianness },
        glycin_common::ChannelType::F32 => JxlDataFormat::F32 { endianness },
        c => {
            return Err(ProcessError::expected(&format!(
                "Unsupported ChannelType {c:?}"
            )));
        }
    };
    let num_ec = decoder.basic_info().extra_channels.len();
    let pixel_format = JxlPixelFormat {
        color_type,
        color_data_format: Some(color_data_format),
        extra_channel_format: vec![None; num_ec],
    };

    // jxl-rs changes its preferred output profile depending on the pixel format.
    decoder.set_pixel_format(pixel_format).expected_error()?;
    let profile = decoder.output_color_profile();
    let iccp = profile.try_as_icc().map(|icc| icc.into_owned());
    let cicp = if let JxlColorProfile::Simple(c) = profile {
        color_encoding_to_cicp(c)
    } else {
        None
    };

    let num_loops = decoder
        .basic_info()
        .animation
        .as_ref()
        .map(|anim| anim.num_loops as usize)
        .unwrap_or(0);

    let offset = data.len() - data_slice.len();
    let loader_implementation = JxlLoader {
        decoder: Some(decoder),
        scanned_frames,
        num_loops,
        current_loop_idx: 0,
        next_frame: 0,
        data,
        offset,
        memory_format,
        icc_profile: iccp,
        cicp,
    };

    Ok((loader_implementation, image_info))
}

pub fn frame<B: glycin_utils::ByteData>(
    loader: &mut JxlLoader,
    frame_request: FrameRequest,
) -> Result<Frame<B>, ProcessError> {
    let num_frames = loader.scanned_frames.len();
    let looped = loader.next_frame >= num_frames;
    if looped {
        if num_frames == 1 || !frame_request.loop_animation {
            return Err(ProcessError::NoMoreFrames);
        }

        loader.next_frame = 0;
        // Track loop count only when there are finite number of loops.
        if loader.num_loops > 0 {
            loader.current_loop_idx += 1;
        }
    }

    if loader.num_loops > 0 && loader.current_loop_idx >= loader.num_loops {
        return Err(ProcessError::NoMoreFrames);
    }

    // Take the decoder out only when all early return condition has been processed.
    let mut decoder = loader.decoder.take().expected_error()?;
    let (xsize, ysize) = decoder.basic_info().size;

    let frame_idx = loader.next_frame;
    if looped {
        let seek_target = loader.scanned_frames[frame_idx].seek_target;
        decoder.start_new_frame(seek_target);
        loader.offset = seek_target.decode_start_file_offset as usize;
    }

    let mut data_slice = &loader.data[loader.offset..];
    let result = decoder
        .process(&mut data_slice, Some(&mut RayonParallelRunner))
        .expected_error()?;
    let frame_decoder = match result {
        ProcessingResult::Complete { result } => result,
        ProcessingResult::NeedsMoreInput { .. } => {
            return Err(ProcessError::expected(&"unexpected end of file"));
        }
    };

    let bytes_per_pixel = loader.memory_format.n_bytes().usize();
    let mut raw_buffer = Vec::new();
    let buffer_size = ysize
        .checked_mul(xsize)
        .expected_error()?
        .checked_mul(bytes_per_pixel)
        .expected_error()?;
    raw_buffer.try_reserve(buffer_size).expected_error()?;
    raw_buffer.resize(buffer_size, 0u8);
    let duration = frame_decoder.frame_header().duration;

    let mut output_buffer = [JxlOutputBuffer::new(
        &mut raw_buffer,
        ysize,
        xsize * bytes_per_pixel,
    )];
    let result = frame_decoder
        .process(
            &mut data_slice,
            &mut output_buffer,
            Some(&mut RayonParallelRunner),
        )
        .expected_error()?;
    decoder = match result {
        ProcessingResult::Complete { result } => result,
        ProcessingResult::NeedsMoreInput { .. } => {
            return Err(ProcessError::expected(&"unexpected end of file"));
        }
    };
    loader.decoder = Some(decoder);

    let texture = B::try_from_vec(raw_buffer).expected_error()?;

    // xsize and ysize are checked to be in u32 range, in `load`.
    let mut frame =
        Frame::new(xsize as u32, ysize as u32, loader.memory_format, texture).expected_error()?;
    frame.details.color_icc_profile = loader
        .icc_profile
        .as_deref()
        .map(B::try_from_slice)
        .transpose()
        .expected_error()?;
    frame.details.color_cicp = loader.cicp.map(|x| x.to_bytes());
    if let Some(ms) = duration {
        frame.delay = Some(std::time::Duration::from_secs_f64(ms / 1000.0)).into();
        frame.details.n_frame = Some(frame_idx as u64);
    }

    loader.next_frame += 1;
    loader.offset = loader.data.len() - data_slice.len();
    Ok(frame)
}

fn color_encoding_to_cicp(c: &JxlColorEncoding) -> Option<Cicp> {
    let (color_primaries, transfer_function) = match c {
        JxlColorEncoding::RgbColorSpace {
            white_point,
            primaries,
            transfer_function,
            ..
        } => {
            let color_primaries = match (white_point, primaries) {
                (JxlWhitePoint::D65, JxlPrimaries::SRGB) => ColorPrimaries::Srgb,
                (JxlWhitePoint::DCI, JxlPrimaries::P3) => ColorPrimaries::DciP3,
                (JxlWhitePoint::D65, JxlPrimaries::P3) => ColorPrimaries::DisplayP3,
                (JxlWhitePoint::D65, JxlPrimaries::BT2100) => ColorPrimaries::Rec2020,
                _ => return None,
            };
            (color_primaries, transfer_function)
        }
        JxlColorEncoding::GrayscaleColorSpace {
            white_point: JxlWhitePoint::D65,
            transfer_function: tf @ (JxlTransferFunction::HLG | JxlTransferFunction::PQ),
            ..
        } => (ColorPrimaries::DisplayP3, tf),
        _ => return None,
    };

    let transfer_characteristics = match transfer_function {
        JxlTransferFunction::Linear => TransferCharacteristics::Linear,
        JxlTransferFunction::HLG => TransferCharacteristics::Hlg,
        JxlTransferFunction::PQ => TransferCharacteristics::Pq,
        JxlTransferFunction::SRGB => TransferCharacteristics::Srgb,
        JxlTransferFunction::BT709 => TransferCharacteristics::Bt709,
        JxlTransferFunction::DCI => TransferCharacteristics::Dci,
        _ => return None,
    };

    Some(Cicp {
        color_primaries,
        matrix_coefficients: MatrixCoefficients::Identity,
        transfer_characteristics,
        video_full_range_flag: gufo_common::cicp::VideoRangeFlag::Full,
    })
}

fn memory_format(
    is_grayscale: bool,
    bit_depth: &JxlBitDepth,
    alpha: Option<&JxlExtraChannel>,
) -> MemoryFormat {
    let alpha_associated = alpha.map(|ec| ec.alpha_associated);

    if matches!(
        bit_depth,
        JxlBitDepth::Int {
            bits_per_sample: ..=16
        }
    ) {
        let sixteen_bits = bit_depth.bits_per_sample() > 8;
        match (is_grayscale, sixteen_bits, alpha_associated) {
            (false, false, None) => MemoryFormat::R8g8b8,
            (false, false, Some(false)) => MemoryFormat::R8g8b8a8,
            (false, false, Some(true)) => MemoryFormat::R8g8b8a8Premultiplied,
            (false, true, None) => MemoryFormat::R16g16b16,
            (false, true, Some(false)) => MemoryFormat::R16g16b16a16,
            (false, true, Some(true)) => MemoryFormat::R16g16b16a16Premultiplied,
            (true, false, None) => MemoryFormat::G8,
            (true, false, Some(false)) => MemoryFormat::G8a8,
            (true, false, Some(true)) => MemoryFormat::G8a8Premultiplied,
            (true, true, None) => MemoryFormat::G16,
            (true, true, Some(false)) => MemoryFormat::G16a16,
            (true, true, Some(true)) => MemoryFormat::G16a16Premultiplied,
        }
    } else if matches!(
        bit_depth,
        JxlBitDepth::Float {
            bits_per_sample: ..=16,
            exponent_bits_per_sample: ..=5
        }
    ) {
        match alpha_associated {
            None => MemoryFormat::R16g16b16Float,
            Some(false) => MemoryFormat::R16g16b16a16Float,
            // f16 memory format doesn't have premultiplied variant
            Some(true) => MemoryFormat::R32g32b32a32FloatPremultiplied,
        }
    } else {
        match alpha_associated {
            None => MemoryFormat::R32g32b32Float,
            Some(false) => MemoryFormat::R32g32b32a32Float,
            Some(true) => MemoryFormat::R32g32b32a32FloatPremultiplied,
        }
    }
}

pub struct RayonParallelRunner;

impl JxlParallelRunner for RayonParallelRunner {
    fn run(&mut self, num: usize, fun: &JxlParallelRunnerFun) -> jxl::error::Result<()> {
        (0..num).into_par_iter().try_for_each(fun)
    }

    fn num_threads(&self) -> usize {
        rayon::current_num_threads()
    }
}
