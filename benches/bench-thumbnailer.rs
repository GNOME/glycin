mod utils;

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use gio::prelude::FileExt;

fn thumbnailer(c: &mut Criterion) {
    for image_path in utils::test_images() {
        let mut group = c.benchmark_group("glycin-thumbnailer");

        let uri = gio::File::for_path(&image_path).uri();

        group.bench_function(utils::bench_name(&image_path), |b| {
            b.iter(|| {
                glycin_thumbnailer::main(black_box(vec![
                    "glycin-thumbnailer".into(),
                    format!("--input={uri}"),
                    "--output=/dev/null".into(),
                    "--size=512".into(),
                ]))
            })
        });
    }
}

criterion_main!(benches);
criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_secs(10)).with_plots();
    targets = thumbnailer
);
