// SPDX-License-Identifier: MPL-2.0 OR LGPL-2.1-or-later

use glycin::{EditOutcome, Editor, Operation, Operations};
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

    let rotate = Operation::Rotate(gufo_common::orientation::Rotation::_90);
    let operations = Operations::new(vec![rotate]);

    let result = Editor::new(file.clone())
        .edit()
        .await?
        .apply_sparse(&operations)
        .await
        .expect("request failed");

    assert_eq!(result.apply_to(file).await.unwrap(), EditOutcome::Changed);

    Ok(())
}
