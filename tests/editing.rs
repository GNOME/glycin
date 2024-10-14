use std::path::{Path, PathBuf};

mod utils;

use utils::*;

#[test]
fn editing_rotation_90() {
    run_test("rotation-90");
}

#[test]
fn editing_crop() {
    run_test("crop");
}

fn run_test(test_name: &str) {
    init();

    block_on(test(test_name))
}

async fn test(test_name: &str) {
    println!("Running test '{test_name}'");

    let base_path = PathBuf::from_iter(["test-images", "editing"]);

    let mut folder = base_path.clone();
    folder.push(test_name);

    let mut reference_path = base_path.clone();
    reference_path.push(format!("{test_name}.png"));

    let mut operations_path = base_path.clone();
    operations_path.push(format!("{test_name}.yml"));

    let mut results = Vec::new();

    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        let edited = async_io::block_on(apply_operations(&path, &operations_path));

        eprintln!("- {path:?}");

        match edited {
            glycin::SparseEdit::Complete(x) => {
                let out_name = format!(
                    "{}-test-out.png",
                    path.file_name().unwrap().to_string_lossy()
                );
                let out_path = write_tmp(&format!("{out_name}-test-out.png"), &x.get().unwrap());
                let result = compare_images_path(&reference_path, out_path, true).await;

                results.push(result);
            }
            _ => {
                todo!()
            }
        }
    }

    TestResult::check_multiple(results);
}

fn write_tmp(path: impl AsRef<Path>, data: &[u8]) -> PathBuf {
    let mut tmp_path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    tmp_path.push(path.as_ref());
    std::fs::write(&tmp_path, data).unwrap();
    tmp_path
}

async fn apply_operations(image: &Path, operations: &Path) -> glycin::SparseEdit {
    let reader = std::fs::File::open(operations).unwrap();
    let operations: glycin::Operations = serde_yml::from_reader(reader).unwrap();

    let file = gio::File::for_path(image);
    let editor = glycin::Editor::new(file);

    editor.apply_sparse(operations).await.unwrap()
}
