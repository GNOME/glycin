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
fn create_jpeg_quality() {
    block_on(async {
        init();

        let width = 3;
        let height = 1;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let data = vec![255, 0, 0, 150, 0, 0, 50, 0, 0];

        let new_image = NewImage::new(width, height, memory_format, data.clone()).unwrap();
        let mut creator = Creator::new(MimeType::jpeg());
        creator.set_quality(100).unwrap();
        let encoded_image = creator.create(new_image).await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full().unwrap());
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[3].abs_diff(data[3]) < 5);

        let new_image = NewImage::new(width, height, memory_format, data.clone()).unwrap();
        let mut creator = Creator::new(MimeType::jpeg());
        creator.set_quality(50).unwrap();
        let encoded_image = creator.create(new_image).await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full().unwrap());
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[3].abs_diff(data[3]) > 5);
    });
}

#[test]
fn create_png_compression() {
    block_on(async {
        init();

        let loader = glycin::Loader::new(gio::File::for_path("test-images/images/color.png"));
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();
        let data = frame.buf_slice().to_vec();

        let width = frame.width();
        let height = frame.height();
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let new_image = NewImage::new(width, height, memory_format, data.clone()).unwrap();
        let mut creator = Creator::new(MimeType::png());
        creator.set_compression(100).unwrap();
        let encoded_image = creator.create(new_image).await.unwrap();

        let size_100 = encoded_image.data_ref().unwrap().len();

        let new_image = NewImage::new(width, height, memory_format, data.clone()).unwrap();
        let mut creator = Creator::new(MimeType::png());
        creator.set_compression(50).unwrap();
        let encoded_image = creator.create(new_image).await.unwrap();

        let size_50 = encoded_image.data_ref().unwrap().len();

        let new_image = NewImage::new(width, height, memory_format, data.clone()).unwrap();
        let mut creator = Creator::new(MimeType::png());
        creator.set_compression(0).unwrap();
        let encoded_image = creator.create(new_image).await.unwrap();

        let size_0 = encoded_image.data_ref().unwrap().len();

        assert!(size_100 < size_50);
        assert!(size_50 < size_0);
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
