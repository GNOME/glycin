use gdk::prelude::*;
use gio::glib;
use glycin_common::MemoryFormatInfo;
use tracing_subscriber::prelude::*;

fn main() {
    glib::MainContext::new().block_on(run()).unwrap();
}

async fn run() -> Result<(), glycin::Error> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::builder().from_env_lossy())
        .with(tracing_subscriber::fmt::Layer::default().compact())
        .init();

    let mut args = std::env::args();
    let bin = args.next().unwrap();
    let Some(path) = args.next() else {
        eprintln!("Usage: {bin} <IMAGE PATH> [NUMBER FRAMES]");
        std::process::exit(2);
    };
    let n_frames = args.next().and_then(|x| x.parse().ok()).unwrap_or(1);

    let file = gio::File::for_path(path);
    let mut image = glycin::Loader::new(file).load().await?;

    let info = image.details();

    println!("[info]");
    println!("dimensions = {} x {}", info.width(), info.height());
    println!(
        "format_name = {}",
        info.info_format_name().as_ref().cloned().unwrap_or("-")
    );
    println!(
        "exif = {}",
        info.metadata_exif()
            .as_ref()
            .map_or(String::from("empty"), |x| glib::format_size(x.len() as u64)
                .to_string())
    );
    println!(
        "xmp = {}",
        info.metadata_xmp()
            .as_ref()
            .map_or(String::from("empty"), |x| glib::format_size(x.len() as u64)
                .to_string())
    );
    if let Some(key_value) = &info.metadata_key_value() {
        println!("key_value = ");
        for (key, value) in *key_value {
            println!(" - {key}: {value}");
        }
    } else {
        println!("key_value = -");
    }
    println!(
        "dimensions_text = {}",
        info.info_dimensions_text().as_ref().cloned().unwrap_or("-")
    );

    for _ in 0..n_frames {
        let frame = image.next_frame().await.unwrap();
        let texture = frame.texture();
        println!("[[frame]]");
        println!("dimensions = {} x {}", frame.width(), frame.height());
        println!(
            "stride = {} ({} px)",
            frame.stride(),
            frame.stride() as f32 / frame.memory_format().n_bytes().u8() as f32
        );
        println!(
            "format = {:?} ({:?})",
            texture.format(),
            frame.memory_format()
        );
        println!(
            "delay = {}",
            frame
                .delay()
                .map(|x| format!("{:#?}", x))
                .unwrap_or("-".into())
        );

        println!(
            "iccp = {}",
            frame
                .details()
                .color_icc_profile()
                .as_ref()
                .map_or(String::from("empty"), |x| glib::format_size(x.len() as u64)
                    .to_string())
        );
        println!(
            "cicp = {}",
            frame
                .details()
                .color_cicp()
                .as_ref()
                .map_or(String::from("empty"), |x| format!("{x:?}"))
        );
        println!(
            "bit_depth = {}",
            frame
                .details()
                .info_bit_depth()
                .map(|x| format!("{} bit", x))
                .unwrap_or("-".into())
        );
        println!(
            "alpha_channel = {}",
            frame
                .details()
                .info_alpha_channel()
                .map(|x| x.to_string())
                .unwrap_or("-".into())
        );
        println!(
            "grayscale = {}",
            frame
                .details()
                .info_grayscale()
                .map(|x| x.to_string())
                .unwrap_or("-".into())
        );
        println!(
            "pixel_density = {}",
            frame
                .details()
                .pixel_density()
                .as_ref()
                .map(|dim| format!("{} ({})", dim.display(), dim.dpi().display()))
                .unwrap_or("-".into())
        );

        println!(
            "physical_size = {}",
            frame
                .details()
                .physical_size()
                .as_ref()
                .map(|dim| dim.display().to_string())
                .unwrap_or("-".into())
        );
    }

    Ok(())
}
