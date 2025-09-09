// SPDX-License-Identifier: MPL-2.0 OR LGPL-2.1-or-later

use gdk::prelude::*;
use glycin::{Loader, MemoryFormatSelection};
use tracing_subscriber::prelude::*;

fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::builder().from_env_lossy())
        .with(tracing_subscriber::fmt::Layer::default().compact())
        .init();

    let Some(path) = std::env::args().nth(1) else {
        std::process::exit(2)
    };

    let _ = async_io::block_on(render(&path));
}

async fn render<P>(path: P) -> Result<(), Box<dyn std::error::Error>>
where
    P: AsRef<std::path::Path>,
{
    let file = gio::File::for_path(path);
    let mut loader = Loader::new(file);
    loader.accepted_memory_formats(MemoryFormatSelection::R8g8b8a8);
    let image = loader.load().await.expect("request failed");
    let frame = image.next_frame().await.expect("next frame failed");

    frame.texture().save_to_png("output.png")?;
    Ok(())
}
