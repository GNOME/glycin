// SPDX-License-Identifier: MPL-2.0 OR LGPL-2.1-or-later

use gdk::prelude::*;
use glycin::Loader;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        std::process::exit(2)
    };

    let _ = async_global_executor::block_on(render(&path));
}

async fn render<P>(path: P) -> Result<(), Box<dyn std::error::Error>>
where
    P: AsRef<std::path::Path>,
{
    let file = gio::File::for_path(path);
    let image = Loader::new(file).load().await.expect("request failed");
    let frame = image.next_frame().await.expect("next frame failed");

    frame.texture().unwrap().save_to_png("output.png")?;
    Ok(())
}
