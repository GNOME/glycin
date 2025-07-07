use std::path::{Path, PathBuf};

mod utils;

use gio::prelude::FileExt;
use glycin::{BinaryData, SparseEdit};
use utils::*;

#[test]
fn editing_rotation_90() {
    run_test("rotation-90");
}

#[test]
fn editing_crop() {
    run_test("crop");
}

#[test]
fn editing_crop_too_large_value() {
    run_test("crop-too-large-value");
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
        eprintln!("- {path:?}");

        let data_sparse = async_io::block_on(apply_operations_sparse(&path, &operations_path));

        let data_complete = async_io::block_on(apply_operations_complete(&path, &operations_path));

        for (apply_type, data) in [("sparse", data_sparse), ("complete", data_complete)] {
            let out_name = format!(
                "{}-{apply_type}-test-out.png",
                path.file_name().unwrap().to_string_lossy()
            );
            let out_path = write_tmp(&format!("{out_name}"), &data.get().unwrap());
            let result = compare_images_path(&reference_path, out_path, true).await;

            results.push(result);
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

async fn apply_operations_sparse(image: &Path, operations: &Path) -> glycin::BinaryData {
    let reader = std::fs::File::open(operations).unwrap();
    let operations: glycin::Operations = serde_yaml::from_reader(reader).unwrap();

    let file = gio::File::for_path(image);
    let editor = glycin::Editor::new(file).edit().await.unwrap();

    let sparse_edit = editor.apply_sparse(&operations).await.unwrap();

    if let SparseEdit::Complete(data) = sparse_edit {
        data
    } else {
        let (tmp_file, _) = gio::File::new_tmp(None::<PathBuf>).unwrap();
        let tmp_path = tmp_file.path().unwrap();

        let original = std::fs::read(&image).unwrap();
        std::fs::write(tmp_path, original).unwrap();

        assert_eq!(
            sparse_edit.apply_to(tmp_file.clone()).await.unwrap(),
            glycin::EditOutcome::Changed
        );

        let data = std::fs::read(tmp_file.path().unwrap()).unwrap();
        BinaryData::from_data(data).unwrap()
    }
}

async fn apply_operations_complete(image: &Path, operations: &Path) -> glycin::BinaryData {
    let reader = std::fs::File::open(operations).unwrap();
    let operations: glycin::Operations = serde_yaml::from_reader(reader).unwrap();

    let file = gio::File::for_path(image);
    let editor = glycin::Editor::new(file).edit().await.unwrap();

    editor.apply_complete(&operations).await.unwrap().data()
}
