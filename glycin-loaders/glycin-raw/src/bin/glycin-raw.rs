// SPDX-Copyright: 2024 Hubert Figuière

use std::io::Read;

use glycin_utils::safe_math::SafeConversion;
use glycin_utils::*;
use libopenraw::metadata::Value;
use libopenraw::{Bitmap, RawImage};

init_main_loader!(ImgDecoder);

pub struct ImgDecoder {
    rawimage: RawImage,
}

pub fn render<B: ByteData>(rawdata: &libopenraw::RawImage) -> Result<Frame<B>, ProcessError> {
    let rawimage = rawdata
        .rendered_image(&libopenraw::RenderingOptions::default())
        .expected_error()?;
    let width = rawimage.width();
    let height = rawimage.height();

    let data8 = if let Some(data16) = rawimage.data16() {
        unsafe {
            std::slice::from_raw_parts(data16.as_ptr().cast::<u8>(), std::mem::size_of_val(data16))
        }
    } else {
        rawimage.data8().expected_error()?
    };

    let texture = B::try_from_slice(data8).internal_error()?;

    Frame::new(
        width.try_u32()?,
        height.try_u32()?,
        MemoryFormat::R16g16b16,
        texture,
    )
    .internal_error()
}

impl LoaderImplementation for ImgDecoder {
    fn load<B: ByteData, S: Read>(
        mut stream: S,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<(ImgDecoder, ImageDetails<B>), ProcessError> {
        let mut buf = vec![];
        stream.read_to_end(&mut buf).internal_error()?;
        let rawfile = libopenraw::rawfile_from_memory(buf, None).expected_error()?;
        let rawimage = rawfile.raw_data(false).expected_error()?;
        let w = rawimage.width();
        let h = rawimage.height();
        let xmp = rawfile
            .metadata_value(&"Exif.Image.ApplicationNotes".to_string())
            .and_then(|value| {
                if let Value::Bytes(xmp) = value {
                    Some(xmp)
                } else {
                    None
                }
            });
        let orientation = rawfile.orientation();

        let mut image_info = ImageDetails::new(w, h);

        image_info.info_format_name = Some(String::from("RAW"));
        image_info.metadata_xmp = xmp.and_then(|xmp| B::try_from_vec(xmp).ok());
        image_info.transformation_orientation = orientation
            .try_into()
            .ok()
            .and_then(|x: u16| gufo_common::orientation::Orientation::try_from(x).ok());
        image_info.transformation_ignore_exif = false;

        let decoder = ImgDecoder { rawimage };

        Ok((decoder, image_info))
    }

    fn specific_frame<B: ByteData>(
        &mut self,
        _frame_request: FrameRequest,
    ) -> Result<Frame<B>, ProcessError> {
        render(&self.rawimage).expected_error()
    }
}
