mod utils;

use std::collections::BTreeMap;

use glycin::{Creator, MimeType, NewImage};
use utils::*;

#[test]
fn write_jpeg() {
    block_on(async {
        init();

        let encoder = Creator::new(MimeType::jpeg());
        let width = 1;
        let height = 1;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let data = vec![255, 0, 0];

        let mut new_image = NewImage::new(width, height, memory_format, data).unwrap();

        new_image.set_color_icc_profile(Some(vec![1, 2, 3]));

        let encoded_image = encoder.create(new_image).await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full().unwrap());
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert_eq!(
            frame.details().iccp.as_ref().unwrap().get_full().unwrap(),
            vec![1, 2, 3]
        );
    });
}

#[test]
fn write_png() {
    block_on(async {
        init();

        let encoder = Creator::new(MimeType::png());
        let width = 1;
        let height = 1;
        let memory_format = glycin::MemoryFormat::B8g8r8;
        let data = vec![0, 0, 255];

        let mut new_image = NewImage::new(width, height, memory_format, data).unwrap();
        new_image.set_key_value(BTreeMap::from_iter(vec![(
            "keyword".to_string(),
            "value".to_string(),
        )]));
        new_image.set_color_icc_profile(Some(vec![1, 2, 3]));

        let encoded_image = encoder.create(new_image).await.unwrap();

        let mut loader = glycin::Loader::new_vec(encoded_image.data_full().unwrap());
        loader.accepted_memory_formats(glycin::MemoryFormatSelection::R8g8b8);
        let image = loader.load().await.unwrap();

        assert_eq!(
            image.info().key_value.as_ref().unwrap().get("keyword"),
            Some(&"value".to_string())
        );

        let frame = image.next_frame().await.unwrap();

        assert_eq!(frame.buf_slice(), [255, 0, 0]);
        assert_eq!(
            frame.details().iccp.as_ref().unwrap().get_full().unwrap(),
            vec![1, 2, 3]
        );
    });
}
