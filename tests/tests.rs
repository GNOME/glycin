use std::path::Path;

use gio::prelude::FileExt;
use utils::*;

mod utils;

#[test]
fn color() {
    test_dir("test-images/images/color");
}

#[test]
fn color_exif_orientation() {
    test_dir_no_exif("test-images/images/color-exif-orientation");
}

#[test]
fn color_iccp_pro() {
    test_dir("test-images/images/color-iccp-pro");
}

#[test]
fn cicp_p3() {
    test_dir("test-images/images/cicp-p3");
}

#[test]
fn gray_iccp() {
    test_dir("test-images/images/gray-iccp");
}

#[test]
fn icon() {
    test_dir("test-images/images/icon");
}

#[test]
fn exif() {
    test_dir("test-images/images/exif");
}

#[test]
fn fonts() {
    test_dir("test-images/images/fonts");
}

#[test]
fn animated_numbers() {
    block_on(test_dir_animated("test-images/images/animated-numbers"));
}

#[test]
fn input_stream() {
    block_on(test_input_stream());
}

fn test_dir(dir: impl AsRef<Path>) {
    block_on(test_dir_options(dir, true));
}

fn test_dir_no_exif(dir: impl AsRef<Path>) {
    block_on(test_dir_options(dir, false));
}

async fn test_dir_animated(dir: impl AsRef<Path>) {
    init();

    let images = std::fs::read_dir(&dir).unwrap();

    for entry in images {
        let path = entry.unwrap().path();
        eprintln!("  - {path:?}");

        if skip_file(&path) {
            eprintln!("    (skipped)");
            continue;
        }

        let file = gio::File::for_path(&path);
        let mut image_request = glycin::Loader::new(file);
        image_request.use_expose_base_dir(true);
        let image = image_request.load().await.unwrap();

        for n_frame in [0, 1, 2, 3, 0, 1, 2, 3] {
            let reference_path = reference_image_path(&dir, Some(n_frame));

            let frame = loop {
                let frame = image.next_frame().await.unwrap();
                if frame.details().n_frame().unwrap() == n_frame {
                    break frame;
                }
            };

            let data = texture_to_bytes(&frame.texture());
            let result = compare_images(reference_path, &path, &data, false).await;

            if result.is_failed() {
                eprintln!("Frame failed: {result:#?}");
                panic!();
            } else {
                eprintln!("{n_frame}    (OK)");
            }
        }

        assert!(image
            .specific_frame(glycin::FrameRequest::default().loop_animation(false))
            .await
            .unwrap_err()
            .is_no_more_frames());
    }
}

async fn test_dir_options(dir: impl AsRef<Path>, exif: bool) {
    init();

    let images = std::fs::read_dir(&dir).unwrap();

    let reference_path = reference_image_path(&dir, None);

    let mut results = Vec::new();
    for entry in images {
        let path = entry.unwrap().path();
        eprintln!("  - {path:?}");

        if skip_file(&path) {
            eprintln!("    (skipped)");
            continue;
        }

        let result = compare_images_path(&reference_path, &path, exif).await;

        results.push(result);
    }

    TestResult::check_multiple(results);
}

async fn test_input_stream() {
    let stream = gio::File::for_path("test-images/images/color/color.jpg")
        .read(gio::Cancellable::NONE)
        .unwrap();
    let loader = unsafe { glycin::Loader::new_stream(stream) };
    let image = loader.load().await.unwrap();

    assert_eq!(image.details().width(), 600);

    let data = std::fs::read("test-images/images/color/color.jpg").unwrap();
    let loader = glycin::Loader::new_vec(data);
    let image = loader.load().await.unwrap();

    assert_eq!(image.details().width(), 600);
}
