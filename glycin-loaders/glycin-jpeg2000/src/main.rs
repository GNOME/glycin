use std::io::Read;

use glycin_utils::*;

init_main_loader!(ImgDecoder);

pub struct ImgDecoder {
    pub image: jpeg2k::Image,
}

unsafe impl Sync for ImgDecoder {}
unsafe impl Send for ImgDecoder {}

impl LoaderImplementation for ImgDecoder {
    fn init(
        mut stream: UnixStream,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<(Self, ImageDetails), ProcessError> {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).internal_error()?;

        let image = jpeg2k::Image::from_bytes(&buf).expected_error()?;
        let details = ImageDetails::new(image.width(), image.height());

        Ok((Self { image }, details))
    }

    fn frame(&mut self, _frame_request: FrameRequest) -> Result<Frame, ProcessError> {
        let dynamic_image = image::DynamicImage::try_from(&self.image).internal_error()?;

        let memory_format =
            glycin_utils::image_rs::memory_format_from_color_type(dynamic_image.color());
        let width = dynamic_image.width();
        let height = dynamic_image.height();

        let texture = BinaryData::from_data(dynamic_image.into_bytes()).internal_error()?;

        Ok(Frame::new(width, height, memory_format, texture).expected_error()?)
    }
}
