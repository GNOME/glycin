use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{Criterion, SamplingMode, criterion_group, criterion_main};
use gio::prelude::FileExt;
use glycin::{Pool, PoolConfig};

fn test_images() -> Vec<std::path::PathBuf> {
    let mut paths = vec![
        PathBuf::from("test-images/images/color/color.avif"),
        PathBuf::from("test-images/images/color/color.jxl"),
        PathBuf::from("test-images/images/color/color.jpg"),
        PathBuf::from("test-images/images/color/color.png"),
        PathBuf::from("test-images/images/color/color.svg"),
        PathBuf::from("test-images/images/color/color.webp"),
        PathBuf::from("test-images/images/tiny/tiny.png"),
    ];

    let download = [
        (
            "gnome-background-50-blendpills-l.jxl",
            "https://gitlab.gnome.org/GNOME/gnome-backgrounds/-/raw/00bdf1cb/backgrounds/blendpills-l.jxl",
        ),
        (
            "gnome-background-50-morphogenesis-l.svg",
            "https://gitlab.gnome.org/GNOME/gnome-backgrounds/-/raw/00bdf1cb/backgrounds/morphogenesis-l.svg",
        ),
    ];

    if !Path::new("cache").is_dir() {
        std::fs::create_dir("cache").unwrap();
    }

    for (filename, url) in download {
        let path = PathBuf::from(format!("cache/{filename}"));
        if !Path::new(&path).is_file() {
            eprintln!("Downloading image from <{url}> …");
            std::process::Command::new("curl")
                .args([url, "--output"])
                .arg(&path)
                .status()
                .unwrap();
        }

        paths.push(path);
    }

    paths
}

fn loader(c: &mut Criterion) {
    for image_path in test_images() {
        let mut group = c.benchmark_group(bench_name(&image_path));

        group.sampling_mode(SamplingMode::Flat);

        group.bench_function("glycin/sandboxed-fresh-pool)", |b| {
            b.iter(|| do_glycin_load_fresh_pool(black_box(&image_path)))
        });
        group.bench_function("glycin/sandboxed", |b| {
            b.iter(|| do_glycin_load(black_box(&image_path)))
        });
        group.bench_function("glycin/unsandboxed", |b| {
            b.iter(|| do_glycin_load_unsandboxed(black_box(&image_path)))
        });
        group.bench_function("GdkPixbuf", |b| {
            b.iter(|| do_gdk_pixbuf_load(black_box(&image_path)))
        });
    }
}

criterion_main!(benches);
criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(100).measurement_time(Duration::from_secs(2)).with_plots();
    targets = loader
);

fn do_glycin_load(path: &Path) {
    glib::MainContext::new().block_on(async {
        let mut loader = glycin::Loader::new(gio::File::for_path(path));
        loader.apply_transformations(false);
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();
        assert!(frame.buf_bytes().len() > 0);
    });
}

fn do_glycin_load_fresh_pool(path: &Path) {
    glib::MainContext::new().block_on(async {
        let mut loader = glycin::Loader::new(gio::File::for_path(path));
        loader.pool(Pool::new(PoolConfig::new()));
        loader.apply_transformations(false);
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();
        assert!(frame.buf_bytes().len() > 0);
    });
}

fn do_glycin_load_unsandboxed(path: &Path) {
    glib::MainContext::new().block_on(async {
        let mut loader = glycin::Loader::new(gio::File::for_path(path));
        loader.sandbox_selector(glycin::SandboxSelector::NotSandboxed);
        loader.apply_transformations(false);
        let image = loader.load().await.unwrap();
        let frame = image.next_frame().await.unwrap();
        assert!(frame.buf_bytes().len() > 0);
    });
}

fn do_gdk_pixbuf_load(path: &Path) {
    glib::MainContext::new().block_on(async {
        let stream = gio::File::for_path(path)
            .read_future(glib::Priority::DEFAULT)
            .await
            .unwrap();
        let pixbuf = gdk_pixbuf::Pixbuf::from_stream_future(&stream)
            .await
            .unwrap();
        assert!(pixbuf.pixel_bytes().unwrap().len() > 0);
    });
}

fn bench_name(path: &Path) -> String {
    path.file_name().unwrap().display().to_string()
}
