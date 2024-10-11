use std::path::{Path, PathBuf};

use glycin::{Operation, Operations};

#[test]
fn testa() {
    eprintln!(
        "{}",
        serde_yml::to_string(&Operations::new(vec![Operation::Clip((3, 4, 15, 10))])).unwrap()
    );

    test("rotation-90");
    test("crop");
}

fn test(test_name: &str) {
    let base_path = PathBuf::from_iter(["test-images", "editing"]);

    let mut folder = base_path.clone();
    folder.push(test_name);

    let mut reference_path = base_path.clone();
    reference_path.push(format!("{test_name}.png"));

    let mut operations_path = base_path.clone();
    operations_path.push(format!("{test_name}.yml"));

    for entry in std::fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        let edited = async_io::block_on(apply_operations(&path, &operations_path));

        match edited {
            glycin::SparseEdit::Complete(x) => {
                let xx = dbg!(format!(
                    "{}-test-out.png",
                    path.file_name().unwrap().to_string_lossy()
                ));
                std::fs::write(format!("{xx}-test-out.png"), x.get().unwrap()).unwrap();
            }
            _ => {
                todo!()
            }
        }
    }
}

async fn apply_operations(image: &Path, operations: &Path) -> glycin::SparseEdit {
    let reader = std::fs::File::open(operations).unwrap();
    let operations: glycin::Operations = serde_yml::from_reader(reader).unwrap();

    let file = gio::File::for_path(image);
    let editor = glycin::Editor::new(file);

    editor.apply_sparse(operations).await.unwrap()
}
