mod utils;

use std::hint::black_box;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use gio::prelude::FileExt;
use glycin::{Pool, PoolConfig};

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
    for image_path in utils::test_images() {
        let mut group = c.benchmark_group(utils::bench_name(&image_path));

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
