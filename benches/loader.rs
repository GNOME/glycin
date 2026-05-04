use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
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

struct Context {
    file: gio::File,
    pool: Arc<Pool>,
    main_loop: glib::MainLoop,
}

impl Context {
    fn new(path: &Path) -> Self {
        let main_context = glib::MainContext::new();
        let main_loop = glib::MainLoop::new(Some(&main_context), false);
        let main_loop_ = main_loop.clone();
        std::thread::spawn(move || {
            main_loop_.run();
        });
        let pool = Pool::new(PoolConfig::new().retention_time(Duration::from_millis(10)));

        let context = Self {
            file: gio::File::for_path(path),
            main_loop,
            pool,
        };

        context
    }

    fn loader(&self) -> glycin::Loader {
        let mut loader = glycin::Loader::new(self.file.clone());
        loader.pool(self.pool.clone());
        loader.main_context_selector(glycin::MainContextSelector::Specific(
            self.main_loop.context(),
        ));
        loader.apply_transformations(false);
        loader
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        self.main_loop.quit();
    }
}

fn loader(c: &mut Criterion) {
    for image_path in test_images() {
        let mut group = c.benchmark_group(bench_name(&image_path));

        group.bench_function("GdkPixbuf", |b| {
            b.iter(|| do_gdk_pixbuf_load(black_box(&image_path)))
        });
        group.bench_function("glycin", |b| {
            b.iter_custom(|iters| {
                let context = Context::new(&image_path);

                for _ in 0..iters {
                    do_glycin_load(black_box(&context))
                }

                let start = std::time::Instant::now();
                for _ in 0..iters {
                    do_glycin_load(black_box(&context))
                }
                let elapsed = start.elapsed();

                drop(context);

                elapsed
            });
        });
    }
}

criterion_main!(benches);
criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(100).measurement_time(Duration::from_secs(2)).with_plots();
    targets = loader
);

fn do_glycin_load(context: &Context) {
    async_io::block_on(async {
        let mut loader = context.loader();
        loader.sandbox_selector(glycin::SandboxSelector::NotSandboxed);

        let mut image = loader.load().await.unwrap();
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
