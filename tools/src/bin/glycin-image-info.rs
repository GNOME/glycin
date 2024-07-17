use gdk::prelude::*;
use gio::glib;

fn main() {
    glib::MainContext::default().block_on(run()).unwrap();
}

async fn run() -> Result<(), glycin::ErrorKind> {
    let mut args = std::env::args();
    let bin = args.next().unwrap();
    let Some(path) = args.next() else {
        eprintln!("Usage: {bin} <IMAGE PATH> [NUMBER FRAMES]");
        std::process::exit(2);
    };
    let n_frames = args.next().and_then(|x| x.parse().ok()).unwrap_or(1);

    let file = gio::File::for_path(path);
    let image = glycin::Loader::new(file).load().await?;

    let info = image.info();

    println!("[info]");
    println!("dimensions = {} x {}", info.width, info.height);
    println!(
        "format_name = {}",
        info.details
            .format_name
            .as_ref()
            .cloned()
            .unwrap_or("-".into())
    );
    println!(
        "exif = {}",
        info.details
            .exif
            .as_ref()
            .map_or(String::from("empty"), |x| glib::format_size(
                x.get_full().unwrap().len() as u64
            )
            .to_string())
    );
    println!(
        "xmp = {}",
        info.details
            .xmp
            .as_ref()
            .map_or(String::from("empty"), |x| glib::format_size(
                x.get_full().unwrap().len() as u64
            )
            .to_string())
    );
    println!(
        "dimensions_text = {}",
        info.details
            .dimensions_text
            .as_ref()
            .cloned()
            .unwrap_or(String::from("-"))
    );
    println!(
        "dimensions_inch = {}",
        info.details
            .dimensions_inch
            .as_ref()
            .map(|(x, y)| format!("{:.3}” x {:.3}”", x, y))
            .unwrap_or("-".into())
    );

    for _ in 0..n_frames {
        let frame = image.next_frame().await.unwrap();
        let texture = frame.texture();
        println!("[[frame]]");
        println!("dimensions = {} x {}", frame.width(), frame.height());
        println!("format = {:?}", texture.format());
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
                .iccp
                .as_ref()
                .map_or(String::from("empty"), |x| glib::format_size(
                    x.get_full().unwrap().len() as u64
                )
                .to_string())
        );
        println!(
            "bit_depth = {}",
            frame
                .details()
                .bit_depth
                .map(|x| format!("{} bit", x))
                .unwrap_or("-".into())
        );
        println!(
            "alpha_channel = {}",
            frame
                .details()
                .alpha_channel
                .map(|x| x.to_string())
                .unwrap_or("-".into())
        );
        println!(
            "grayscale = {}",
            frame
                .details()
                .grayscale
                .map(|x| x.to_string())
                .unwrap_or("-".into())
        );
    }

    Ok(())
}
