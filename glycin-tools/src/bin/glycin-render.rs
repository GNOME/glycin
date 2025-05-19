// SPDX-License-Identifier: MPL-2.0 OR LGPL-2.1-or-later

use gdk::prelude::*;
use glycin::{Loader, MemoryFormatSelection};

fn main() {
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
    loader.accepted_memory_formats(MemoryFormatSelection::B8g8r8);
    let image = loader.load().await.expect("request failed");
    let frame = image.next_frame().await.expect("next frame failed");

    frame.texture().save_to_png("output.png")?;
    Ok(())
}
