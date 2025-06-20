mod utils;

use glycin::{Encoder, MimeType, NewImage};
use utils::*;

#[test]
fn write_jpeg() {
    let encoder = Encoder::new();
    let width = 1;
    let height = 1;
    let memory_format = glycin::MemoryFormat::R8g8b8;
    let data = vec![255, 0, 0];

    let new_image = NewImage::new(width, height, memory_format, data).unwrap();
    block_on(encoder.create(new_image, MimeType::jpeg())).unwrap();
}
