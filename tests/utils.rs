#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gdk::prelude::*;
use tracing_subscriber::prelude::*;

pub fn init() {
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::Layer::default().compact())
        .try_init();
}

pub fn reference_image_path(dir: impl AsRef<Path>, frame: Option<u64>) -> PathBuf {
    let mut path = dir.as_ref().to_path_buf();
    if let Some(frame) = frame {
        let mut name = path.file_name().unwrap().to_owned();
        name.push(format!("-{frame}"));
        path.set_file_name(name);
    }
    path.set_extension("png");
    path
}

pub fn skip_file(path: &Path) -> bool {
    extensions_to_skip().contains(&path.extension().unwrap_or_default().into())
}

pub fn extensions_to_skip() -> Vec<OsString> {
    option_env!("GLYCIN_TEST_SKIP_EXT")
        .unwrap_or_default()
        .split(|x| x == ',')
        .map(OsString::from)
        .collect()
}

pub async fn compare_images_path(
    reference_path: impl AsRef<Path>,
    path_compare: impl AsRef<Path>,
    exif: bool,
) -> TestResult {
    let data = get_downloaded_texture(&path_compare).await;
    compare_images(&reference_path, &path_compare, &data, exif).await
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

async fn get_downloaded_texture(path: impl AsRef<Path>) -> Vec<u8> {
    let texture = get_texture(&path).await;
    texture_to_bytes(&texture)
}

pub fn texture_to_bytes(texture: &gdk::Texture) -> Vec<u8> {
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
    let mut loader = glycin::Loader::new(file);
    loader.use_expose_base_dir(true);
    let image = loader.load().await.unwrap();
    let frame = image.next_frame().await.unwrap();
    frame.texture()
}

async fn get_info(path: impl AsRef<Path>) -> glycin::ImageDetails {
    let file = gio::File::for_path(&path);
    let loader = glycin::Loader::new(file);
    let image = loader.load().await.unwrap();
    image.details().clone()
}

pub async fn compare_images(
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
        .metadata_exif()
        .map(|x| x.get().unwrap());

    let exif_eq = if !test_exif
        || (reference_exif.is_none() && path.as_ref().extension().unwrap() == "tiff")
    {
        true
    } else {
        let exif = get_info(&path)
            .await
            .metadata_exif()
            .map(|x| x.get().unwrap());
        reference_exif.as_ref().map(|x| &x[..2]) == exif.as_ref().map(|x| &x[..2])
    };

    TestResult {
        path: path.as_ref().to_path_buf(),
        texture_eq,
        texture_deviation,
        exif_eq,
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct TestResult {
    pub path: PathBuf,
    pub texture_eq: bool,
    pub texture_deviation: f64,
    pub exif_eq: bool,
}

impl TestResult {
    pub fn is_failed(&self) -> bool {
        !self.texture_eq || !self.exif_eq
    }

    pub fn check_multiple(results: Vec<Self>) {
        let mut some_failed = false;
        for result in results.iter() {
            if result.is_failed() {
                some_failed = true;
            } else {
                eprintln!("    (OK)");
            }
        }

        assert!(!some_failed, "{results:#?}");
    }
}
