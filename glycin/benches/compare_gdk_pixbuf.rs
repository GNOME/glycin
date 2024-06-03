use std::{path::Path, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

pub fn criterion_benchmark(c: &mut Criterion) {
    let paths = std::fs::read_dir("../tests/test-images/images/color").unwrap();

    let mut group = c.benchmark_group("Color");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(1));
    group.measurement_time(Duration::from_millis(10));
    group.sampling_mode(criterion::SamplingMode::Flat);

    for entry in paths {
        let rust_path = entry.unwrap().path();
        let extension = rust_path.extension().unwrap().to_str().unwrap();
        if ["dds", "svg", "exr"].contains(&extension) {
            continue;
        }

        let path = rust_path.to_str().unwrap();

        group.bench_with_input(
            BenchmarkId::new("Glycin Sandboxed", extension),
            path,
            |b, path| b.iter(|| run_glycin_sandboxed(path)),
        );

        group.bench_with_input(
            BenchmarkId::new("Glycin Not Sandboxed", extension),
            path,
            |b, path| b.iter(|| run_glycin_not_sandboxed(path)),
        );

        group.bench_with_input(BenchmarkId::new("GdkPixbuf", extension), path, |b, path| {
            b.iter(|| run_gdk_pixbuf(path))
        });
    }
    group.finish();
}

fn run_glycin_sandboxed(path: impl AsRef<Path>) {
    async_io::block_on(async {
        let file = gio::File::for_path(path);
        let loader = glycin::Loader::new(file);
        loader.load().await.unwrap();
    });
}

fn run_glycin_not_sandboxed(path: impl AsRef<Path>) {
    async_io::block_on(async {
        let file = gio::File::for_path(path);
        let mut loader = glycin::Loader::new(file);
        loader.sandbox_mechanism(Some(glycin::SandboxMechanism::NotSandboxed));
        loader.load().await.unwrap();
    });
}

fn run_gdk_pixbuf(path: impl AsRef<Path>) {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).unwrap();
    pixbuf.apply_embedded_orientation().unwrap();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
