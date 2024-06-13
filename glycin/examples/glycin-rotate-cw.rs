// SPDX-License-Identifier: MPL-2.0 OR LGPL-2.1-or-later

use glycin::{EditOutcome, Editor};
use glycin_utils::operations::{Operation, Operations};

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

    let rotate = Operation::Rotate(gufo_common::orientation::Rotation::_90);
    let operations = Operations::new(vec![rotate]);

    let result = Editor::new(file.clone())
        .apply_sparse(operations)
        .await
        .expect("request failed");

    assert_eq!(result.apply_to(file).await.unwrap(), EditOutcome::Changed);

    Ok(())
}
