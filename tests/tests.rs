use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gdk::prelude::*;

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

#[allow(dead_code)]
#[derive(Debug)]
struct TestResult {
    texture_eq: bool,
    texture_deviation: f64,
    exif_eq: bool,
}

impl TestResult {
    fn is_failed(&self) -> bool {
        !self.texture_eq || !self.exif_eq
    }
}

fn test_dir(dir: impl AsRef<Path>) {
    async_global_executor::block_on(test_dir_options(dir, true));
}

fn test_dir_no_exif(dir: impl AsRef<Path>) {
    async_global_executor::block_on(test_dir_options(dir, false));
}

async fn test_dir_options(dir: impl AsRef<Path>, exif: bool) {
    let images = std::fs::read_dir(&dir).unwrap();

    let skip_ext: Vec<_> = option_env!("GLYCIN_TEST_SKIP_EXT")
        .unwrap_or_default()
        .split(|x| x == ',')
        .map(OsString::from)
        .collect();

    let mut reference_path = dir.as_ref().to_path_buf();
    reference_path.set_extension("png");

    let mut some_failed = false;
    let mut list = Vec::new();
    for entry in images {
        let path = entry.unwrap().path();
        eprintln!("  - {path:?}");

        if skip_ext.contains(&path.extension().unwrap_or_default().into()) {
            eprintln!("    (skipped)");
            continue;
        }

        let result = compare_images(&reference_path, &path, exif).await;

        if result.is_failed() {
            some_failed = true;
        } else {
            eprintln!("    (OK)");
        }

        list.push((format!("{path:#?}"), result));
    }

    assert!(!some_failed, "{list:#?}");
}

async fn compare_images(
    reference_path: impl AsRef<Path>,
    path: impl AsRef<Path>,
    test_exif: bool,
) -> TestResult {
    let reference_data = get_downloaded_texture(&reference_path).await;
    let data = get_downloaded_texture(&path).await;

    assert_eq!(reference_data.len(), data.len());

    let len = data.len();

    let mut dev = 0;
    for (r, p) in reference_data.into_iter().zip(&data) {
        dev += (r as i16 - *p as i16).unsigned_abs() as u64;
    }

    let texture_deviation = dev as f64 / len as f64;

    let texture_eq = texture_deviation < 3.1;

    if !texture_eq {
        debug_file(&path).await;
    }

    let reference_exif = get_info(&reference_path)
        .await
        .details
        .exif
        .map(|x| x.get().unwrap());
    let exif = get_info(&path).await.details.exif.map(|x| x.get().unwrap());

    let exif_eq = if !test_exif
        || (reference_exif.is_none() && path.as_ref().extension().unwrap() == "tiff")
    {
        true
    } else {
        reference_exif.as_ref().map(|x| &x[..2]) == exif.as_ref().map(|x| &x[..2])
    };

    TestResult {
        texture_eq,
        texture_deviation,
        exif_eq,
    }
}

async fn get_downloaded_texture(path: impl AsRef<Path>) -> Vec<u8> {
    let texture = get_texture(&path).await;
    let mut data = vec![0; texture.width() as usize * texture.height() as usize * 4];
    texture.download(&mut data, texture.width() as usize * 4);
    data
}

async fn debug_file(path: impl AsRef<Path>) {
    let texture = get_texture(&path).await;
    let mut new_path = PathBuf::from("failures");
    new_path.push(path.as_ref().file_name().unwrap());
    let mut extension = new_path.extension().unwrap().to_os_string();
    extension.push(".png");
    new_path.set_extension(extension);
    texture.save_to_png(new_path).unwrap();
}

async fn get_texture(path: impl AsRef<Path>) -> gdk::Texture {
    let file = gio::File::for_path(&path);
    let image_request = glycin::Loader::new(file);
    let image = image_request.load().await.unwrap();
    let frame = image.next_frame().await.unwrap();
    frame.texture()
}

async fn get_info(path: impl AsRef<Path>) -> glycin::ImageInfo {
    let file = gio::File::for_path(&path);
    let image_request = glycin::Loader::new(file);
    let image = image_request.load().await.unwrap();
    image.info().clone()
}
