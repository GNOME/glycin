use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gdk::prelude::*;
use tracing_subscriber::layer::*;
use tracing_subscriber::util::*;

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

#[test]
fn animated_numbers() {
    block_on(test_dir_animated("test-images/images/animated-numbers"));
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
        let image_request = glycin::Loader::new(file);
        let image = image_request.load().await.unwrap();

        for n_frame in [0, 1, 2, 0] {
            let reference_path = reference_image_path(&dir, Some(n_frame));

            let frame = loop {
                let frame = image.next_frame().await.unwrap();
                if frame.details().n_frame.unwrap() == n_frame {
                    break frame;
                }
            };

            let data = texture_to_bytes(&frame.texture());
            let result = compare_images(reference_path, &path, &data, false).await;

            if result.is_failed() {
                dbg!(result);
                panic!();
            } else {
                eprintln!("{n_frame}    (OK)");
            }
        }
    }
}

async fn test_dir_options(dir: impl AsRef<Path>, exif: bool) {
    init();

    let images = std::fs::read_dir(&dir).unwrap();

    let reference_path = reference_image_path(&dir, None);

    let mut some_failed = false;
    let mut list = Vec::new();
    for entry in images {
        let path = entry.unwrap().path();
        eprintln!("  - {path:?}");

        if skip_file(&path) {
            eprintln!("    (skipped)");
            continue;
        }

        let data = get_downloaded_texture(&path).await;
        let result = compare_images(&reference_path, &path, &data, exif).await;

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
    data: &[u8],
    test_exif: bool,
) -> TestResult {
    let reference_data = get_downloaded_texture(&reference_path).await;

    assert_eq!(reference_data.len(), data.len());

    let len = data.len();

    let mut dev = 0;
    for (r, p) in reference_data.into_iter().zip(data) {
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
    texture_to_bytes(&texture)
}

fn texture_to_bytes(texture: &gdk::Texture) -> Vec<u8> {
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

fn init() {
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::Layer::default().compact())
        .try_init();
}

fn reference_image_path(dir: impl AsRef<Path>, frame: Option<u64>) -> PathBuf {
    let mut path = dir.as_ref().to_path_buf();
    if let Some(frame) = frame {
        let mut name = path.file_name().unwrap().to_owned();
        name.push(format!("-{frame}"));
        path.set_file_name(name);
    }
    path.set_extension("png");
    path
}

fn skip_file(path: &Path) -> bool {
    extensions_to_skip().contains(&path.extension().unwrap_or_default().into())
}

fn extensions_to_skip() -> Vec<OsString> {
    option_env!("GLYCIN_TEST_SKIP_EXT")
        .unwrap_or_default()
        .split(|x| x == ',')
        .map(OsString::from)
        .collect()
}

#[cfg(not(feature = "tokio"))]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    async_io::block_on(future)
}

#[cfg(feature = "tokio")]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::sync::OnceLock;
    static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime =
        TOKIO_RT.get_or_init(|| tokio::runtime::Runtime::new().expect("tokio runtime was created"));
    runtime.block_on(future)
}
