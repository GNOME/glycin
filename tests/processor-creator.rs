mod utils;
use std::collections::BTreeMap;

use glycin::{Creator, Loader, MimeType};
use glycin_core::{self as glycin, MemoryFormat};
use glycin_utils::MemoryFormatInfo;
use utils::*;

#[test]
fn processor_creator_roundtrip_all() {
    block_on(async {
        let reference_path = "test-images/images/color/color.png";

        let loader = Loader::new(gio::File::for_path(reference_path));
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();
        let width = frame.width();
        let height = frame.height();
        let texture = frame.buf_slice();
        let memory_format = frame.memory_format();

        for mime_type in [
            MimeType::AVIF,
            MimeType::BMP,
            MimeType::GIF,
            MimeType::HEIC,
            MimeType::JXL,
            MimeType::JPEG,
            MimeType::PNG,
            MimeType::QOI,
            MimeType::TGA,
            MimeType::TIFF,
            MimeType::WEBP,
        ] {
            if skip_file_ext(mime_type.extension().unwrap()) {
                continue;
            }

            eprintln!("- {}", mime_type.as_str());

            let mut encoder = Creator::new(mime_type.clone()).await.unwrap();
            encoder
                .add_frame(width, height, memory_format, texture.to_vec())
                .unwrap();

            let encoded_image = encoder.create().await.unwrap();

            let path = format!(
                "{}/{}.{}",
                env!("CARGO_TARGET_TMPDIR"),
                mime_type.as_str().replace("/", "-"),
                mime_type.extension().unwrap()
            );
            std::fs::write(&path, encoded_image.data_ref()).unwrap();

            let result = compare_images_path(reference_path, path, false).await;
            if result.is_failed() {
                eprintln!("{result:#?}");
                assert!(false);
            }
        }
    });
}

#[test]
fn processor_creator_jpeg() {
    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::JPEG).await.unwrap();
        let width = 1;
        let height = 1;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let texture = vec![255, 0, 0];

        let frame = encoder
            .add_frame(width, height, memory_format, texture)
            .unwrap();
        frame.set_color_icc_profile(Some(vec![1, 2, 3])).unwrap();

        let encoded_image = encoder.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full());
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert_eq!(
            frame.details().color_icc_profile().unwrap().to_vec(),
            vec![1, 2, 3]
        );
    });
}

#[test]
fn processor_creator_jpeg_stride() {
    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::JPEG).await.unwrap();
        encoder.set_encoding_quality(100).unwrap();
        let width = 2;
        let height = 2;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let texture = vec![
            255, 0, 0, 0, 255, 0, 100, 101, // First line
            0, 0, 255, 25, 50, 75, 102, 103,
        ];

        encoder
            .add_frame_with_stride(width, height, 8, memory_format, texture)
            .unwrap();

        let encoded_image = encoder.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full());
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[2 * 3] < 10);
        assert!(frame.buf_slice()[2 * 3 + 1] < 10);
        assert!(frame.buf_slice()[2 * 3 + 2] > 245);
    });
}

#[test]
fn processor_creator_jpeg_stride_last_row() {
    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::JPEG).await.unwrap();
        encoder.set_encoding_quality(100).unwrap();
        let width = 2;
        let height = 2;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let texture = vec![
            255, 0, 0, 0, 255, 0, 100, 101, // First line with stride
            0, 0, 255, 25, 150, 175,
        ];

        encoder
            .add_frame_with_stride(width, height, 8, memory_format, texture)
            .unwrap();

        let encoded_image = encoder.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full());
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[2 * 3] < 10);
        assert!(frame.buf_slice()[2 * 3 + 1] < 10);
        assert!(frame.buf_slice()[2 * 3 + 2] > 245);
    });
}

#[test]
fn processor_creator_jpeg_stride_invalid() {
    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::JPEG).await.unwrap();
        encoder.set_encoding_quality(100).unwrap();
        let width = 2;
        let height = 2;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let texture = vec![0; 13];

        let res = encoder
            .add_frame_with_stride(width, height, 8, memory_format, texture)
            .unwrap_err();

        assert!(matches!(
            res.kind(),
            glycin::ErrorKind::TextureWrongSize { .. }
        ));
    });
}

#[test]
fn processor_creator_jpeg_quality() {
    block_on(async {
        init();

        let width = 3;
        let height = 1;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let texture = vec![255, 0, 0, 150, 0, 0, 50, 0, 0];

        let mut creator = Creator::new(MimeType::JPEG).await.unwrap();
        creator.set_encoding_quality(100).unwrap();
        creator
            .add_frame(width, height, memory_format, texture.clone())
            .unwrap();
        let encoded_image = creator.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full());
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[3].abs_diff(texture[3]) < 5);

        let mut creator = Creator::new(MimeType::JPEG).await.unwrap();
        creator.set_encoding_quality(50).unwrap();
        creator
            .add_frame(width, height, memory_format, texture.clone())
            .unwrap();
        let encoded_image = creator.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full());
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[3].abs_diff(texture[3]) > 5);
    });
}

#[test]
fn processor_creator_png_compression() {
    block_on(async {
        init();

        let loader = glycin::Loader::new(gio::File::for_path("test-images/images/color.png"));
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();
        let texture = frame.buf_slice().to_vec();

        let width = frame.width();
        let height = frame.height();
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let mut creator = Creator::new(MimeType::PNG).await.unwrap();
        creator.set_encoding_compression(100).unwrap();
        creator
            .add_frame(width, height, memory_format, texture.clone())
            .unwrap();
        let encoded_image = creator.create().await.unwrap();

        let size_100 = encoded_image.data_ref().len();

        let mut creator = Creator::new(MimeType::PNG).await.unwrap();
        creator.set_encoding_compression(50).unwrap();
        creator
            .add_frame(width, height, memory_format, texture.clone())
            .unwrap();
        let encoded_image = creator.create().await.unwrap();

        let size_50 = encoded_image.data_ref().len();

        let mut creator = Creator::new(MimeType::PNG).await.unwrap();
        creator.set_encoding_compression(0).unwrap();
        creator
            .add_frame(width, height, memory_format, texture.clone())
            .unwrap();
        let encoded_image = creator.create().await.unwrap();

        let size_0 = encoded_image.data_ref().len();

        assert!(size_100 < size_50, "{size_100} < {size_50}");
        assert!(size_50 < size_0, "{size_50} < {size_0}");
    });
}

#[test]
fn processor_creator_png() {
    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::PNG).await.unwrap();

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
        new_frame
            .set_color_icc_profile(Some(vec![1, 2, 3]))
            .unwrap();

        let encoded_image = encoder.create().await.unwrap();

        let mut loader = glycin::Loader::new_vec(encoded_image.data_full());
        loader.accepted_memory_formats(glycin::MemoryFormatSelection::R8g8b8);
        let mut image = loader.load().await.unwrap();

        assert_eq!(
            image
                .details()
                .metadata_key_value()
                .as_ref()
                .unwrap()
                .get("keyword"),
            Some(&"value".to_string())
        );

        let frame = image.next_frame().await.unwrap();

        assert_eq!(frame.buf_slice(), [255, 0, 0]);
        assert_eq!(
            frame.details().color_icc_profile().unwrap().to_vec(),
            vec![1, 2, 3]
        );
    });
}

#[test]
fn processor_creator_avif() {
    if skip_file_ext(MimeType::AVIF.extension().unwrap()) {
        return;
    }

    block_on(async {
        init();

        let mut encoder = Creator::new(MimeType::AVIF).await.unwrap();
        encoder.set_encoding_quality(100).unwrap();

        let width = 1;
        let height = 1;
        let memory_format = glycin::MemoryFormat::R8g8b8;
        let texture = vec![255, 0, 0];

        encoder
            .add_frame(width, height, memory_format, texture)
            .unwrap();
        let encoded_image = encoder.create().await.unwrap();

        let loader = glycin::Loader::new_vec(encoded_image.data_full());
        let mut image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();

        assert!(frame.buf_slice()[0] >= 253);
        assert!(frame.buf_slice()[1] <= 2);
        assert!(frame.buf_slice()[2] <= 2);
    });
}

#[test]
fn processor_creator_supported_memory_formats() {
    block_on(async {
        let config = glycin::config::Config::cached().await;

        for (mime_type, c) in config.editors() {
            for memory_format in MemoryFormat::ALL {
                let mut creator = glycin::Creator::new(mime_type.clone()).await.unwrap();
                creator.set_transform_memory_format(false);
                let texture = vec![0; memory_format.n_bytes().usize()];
                creator.add_frame(1, 1, *memory_format, texture).unwrap();
                let result = creator.create().await;

                // Check if creators actually only support the memory formats declared in config
                if c.creator_memory_formats().contains(memory_format) {
                    assert!(
                        result.is_ok(),
                        "Expected encoding support for '{mime_type}' with format '{memory_format:?}'"
                    )
                } else {
                    assert!(
                        result.is_err(),
                        "Expected error for '{mime_type}' with format '{memory_format:?}'"
                    )
                }
            }
        }
    });
}
