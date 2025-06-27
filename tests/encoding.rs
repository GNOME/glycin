mod utils;

use std::collections::BTreeMap;

use glycin::{Creator, MimeType};
use utils::*;

#[test]
fn write_jpeg() {
    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::jpeg()).await.unwrap();
        let width = 1;
        let height = 1;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let texture = vec![255, 0, 0];

        let frame = encoder
            .add_frame(width, height, memory_format, texture)
            .unwrap();
        frame.set_color_icc_profile(Some(vec![1, 2, 3]));

        let encoded_image = encoder.create().await.unwrap();

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
        let texture = vec![255, 0, 0, 150, 0, 0, 50, 0, 0];

        let mut creator = Creator::new(MimeType::jpeg()).await.unwrap();
        creator.set_encoding_quality(100).unwrap();
        creator.add_frame(width, height, memory_format, texture.clone());
        let encoded_image = creator.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full().unwrap());
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[3].abs_diff(texture[3]) < 5);

        let mut creator = Creator::new(MimeType::jpeg()).await.unwrap();
        creator.set_encoding_quality(50).unwrap();
        creator.add_frame(width, height, memory_format, texture.clone());
        let encoded_image = creator.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full().unwrap());
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[3].abs_diff(texture[3]) > 5);
    });
}

#[test]
fn create_png_compression() {
    block_on(async {
        init();

        let loader = glycin::Loader::new(gio::File::for_path("test-images/images/color.png"));
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();
        let texture = frame.buf_slice().to_vec();

        let width = frame.width();
        let height = frame.height();
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let mut creator = Creator::new(MimeType::png()).await.unwrap();
        creator.set_encoding_compression(100).unwrap();
        creator.add_frame(width, height, memory_format, texture.clone());
        let encoded_image = creator.create().await.unwrap();

        let size_100 = encoded_image.data_ref().unwrap().len();

        let mut creator = Creator::new(MimeType::png()).await.unwrap();
        creator.set_encoding_compression(50).unwrap();
        creator.add_frame(width, height, memory_format, texture.clone());
        let encoded_image = creator.create().await.unwrap();

        let size_50 = encoded_image.data_ref().unwrap().len();

        let mut creator = Creator::new(MimeType::png()).await.unwrap();
        creator.set_encoding_compression(0).unwrap();
        creator.add_frame(width, height, memory_format, texture.clone());
        let encoded_image = creator.create().await.unwrap();

        let size_0 = encoded_image.data_ref().unwrap().len();

        assert!(size_100 < size_50);
        assert!(size_50 < size_0);
    });
}

#[test]
fn write_png() {
    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::png()).await.unwrap();

        let width = 1;
        let height = 1;
        let memory_format = glycin::MemoryFormat::B8g8r8;
        let texture = vec![0, 0, 255];

        encoder
            .set_metadata_key_value(BTreeMap::from_iter(vec![(
                "keyword".to_string(),
                "value".to_string(),
            )]))
            .unwrap();
        let new_frame = encoder
            .add_frame(width, height, memory_format, texture)
            .unwrap();
        new_frame.set_color_icc_profile(Some(vec![1, 2, 3]));

        let encoded_image = encoder.create().await.unwrap();

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
