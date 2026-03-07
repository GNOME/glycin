use std::io::{Cursor, Read};

use glycin_utils::safe_math::*;
use glycin_utils::*;

use crate::{FrameSender, ImageRsDecoder, ImageRsFormat, Reader};

pub fn worker(format: ImageRsFormat<Reader>, data: Reader, mime_type: String, send: FrameSender) {
    let mut format = Some(format);

    std::thread::park();

    let mut looped = false;

    // Replay animation from beginning
    loop {
        log::trace!("animated: Start loading loop for {mime_type}");

        if format.is_none() {
            format = ImageRsFormat::create(data.clone(), &mime_type).ok();
        }

        let mut decoder = format.as_mut().map(|x| &mut x.decoder);

        // Use transparent background instead of suggested background color
        if let Some(ImageRsDecoder::WebP(webp)) = &mut decoder {
            let _result = webp.set_background_color(image::Rgba::from([0, 0, 0, 0]));
        }

        let frame_details = match format.as_mut().unwrap().frame_details() {
            Ok(frame_details) => Some(frame_details),
            Err(err) => {
                send.send(Err(err)).unwrap();
                return;
            }
        };

        let mut frames = std::mem::take(&mut format)
            .unwrap()
            .decoder
            .into_frames()
            .unwrap();
        let mut first_frames = Vec::new();

        // Decode first two frames to check if actually an animation
        log::trace!("animated: Decoding first two frames");
        for _ in 0..2 {
            if let Some(frame) = frames.next() {
                first_frames.push(frame);
            }
        }

        let is_animated = match first_frames.len() {
            0 => {
                send.send(Err(ProcessError::expected(&"No frame found.")))
                    .unwrap();
                return;
            }
            1 => false,
            _ => true,
        };

        for frame in first_frames.into_iter().chain(frames).enumerate() {
            // Only use FrameDetails for still images because they might not make too much
            // sense otherwise
            let frame_details = (!is_animated).then(|| frame_details.clone()).flatten();

            let decoded_frame = animated_get_frame(frame, frame_details, is_animated);
            send.send(decoded_frame.map(|x| (x, looped))).unwrap();

            // If not really an animation no need to keep the thread around
            if !is_animated {
                log::debug!("animated: Image is actually not animated");
                return;
            }

            std::thread::park();
        }

        looped = true;
    }
}

pub fn animated_get_frame(
    (n_frame, frame): (usize, Result<image::Frame, image::ImageError>),
    frame_details: Option<FrameDetails>,
    is_animated: bool,
) -> Result<Frame, ProcessError> {
    log::trace!("animated: Treating decoded frame {n_frame}");
    let frame = frame.expected_error()?;

    let (delay_num, delay_den) = frame.delay().numer_denom_ms();

    let delay = if !is_animated {
        None
    } else if delay_num == 0 || delay_den == 0 {
        // Other decoders default to this value as well
        Some(std::time::Duration::from_millis(100))
    } else {
        let micros = f64::round(delay_num as f64 * 1000. / delay_den as f64) as u64;
        Some(std::time::Duration::from_micros(micros))
    };

    let buffer = frame.into_buffer();

    let memory_format = MemoryFormat::R8g8b8a8;
    let width = buffer.width();
    let height = buffer.height();

    let mut memory =
        SharedMemory::new(u64::from(width) * u64::from(height) * memory_format.n_bytes().u64())
            .expected_error()
            .unwrap();
    Cursor::new(buffer.into_raw())
        .read_exact(&mut memory)
        .unwrap();
    let texture = memory.into_binary_data();

    let mut out_frame = Frame::new(width, height, memory_format, texture).unwrap();
    out_frame.delay = delay.into();

    // Set frame info for still pictures
    if let Some(frame_details) = frame_details {
        out_frame.details = frame_details;
    };

    out_frame.details.n_frame = Some(n_frame.try_u64()?);

    Ok(out_frame)
}
