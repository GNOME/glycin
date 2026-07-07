mod utils;

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use glycin::MemoryFormat;

fn images() -> Vec<(MemoryFormat, PathBuf)> {
    let mut img = utils::test_images();

    vec![
        (
            MemoryFormat::G8,
            img.take(Path::new("test-images/images/grayscale/grayscale.jpg"))
                .unwrap(),
        ),
        (
            MemoryFormat::R32g32b32Float,
            img.take(Path::new("test-images/images/color/color.exr"))
                .unwrap(),
        ),
        (
            MemoryFormat::R8g8b8,
            img.take(Path::new("test-images/images/color/color.jpg"))
                .unwrap(),
        ),
        (
            MemoryFormat::B8g8r8a8Premultiplied,
            img.take(Path::new("cache/gnome-bg-50-morphogenesis-l.svg"))
                .unwrap(),
        ),
        (
            MemoryFormat::R16g16b16,
            img.take(Path::new("cache/gnome-bg-50-blendpills-l.jxl"))
                .unwrap(),
        ),
    ]
}

fn convert_image_format(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!("glycin-utils/change_memory_format",));
    for (src_format, image_path) in images() {
        let file = gio::File::for_path(&image_path);

        for target_format in [glycin::MemoryFormat::R8g8b8, glycin::MemoryFormat::R8g8b8a8] {
            group.bench_function(
                format!(
                    "{}/{src_format:?}-to-{target_format:?}",
                    utils::bench_name(&image_path)
                ),
                |b| {
                    b.iter_batched(
                        || {
                            let loader = glycin::Loader::new(file.clone());
                            let mut image = async_io::block_on(loader.load()).unwrap();
                            let frame = async_io::block_on(image.next_frame()).unwrap();

                            assert_eq!(frame.memory_format(), src_format, "{image_path:?}");

                            glycin_utils::Frame::new(
                                frame.width(),
                                frame.height(),
                                frame.memory_format(),
                                glycin_utils::LocalMemory::from(frame.buf_slice().to_vec()),
                            )
                            .unwrap()
                            .into_fungible()
                        },
                        |mut frame| {
                            glycin_utils::editing::change_memory_format(
                                black_box(&mut frame),
                                black_box(target_format),
                            )
                            .unwrap();
                        },
                        criterion::BatchSize::PerIteration,
                    )
                },
            );
        }
    }
}

criterion_main!(benches);
criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::from_millis(500)).with_plots();
    targets = convert_image_format
);
