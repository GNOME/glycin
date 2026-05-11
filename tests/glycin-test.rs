//! Tests that require the glycin-test loader

mod utils;

use std::time::Duration;

use glycin_core::{Limits, MimeType, Operation, Operations};
use utils::*;

fn instruction(instructions: &[&[u8]]) -> Vec<u8> {
    let mut vec = b"glycin-test\0".to_vec();

    vec.extend(instructions.join(&b":"[..]));

    vec
}

#[test]
fn glycin_test_panic_load() {
    init();

    block_on(async {
        let loader = glycin_core::Loader::new_vec(instruction(&[b"panic"]));
        let err = loader.load().await.unwrap_err();
        assert!(err.is_panic(), "Error: {err}");
        #[cfg(all(feature = "builtin-loaders", not(feature = "external-loaders")))]
        assert!(matches!(
            err.kind(),
            glycin_core::ErrorKind::ThreadPanic(Some(_))
        ));
    });
}

#[test]
fn glycin_test_panic_frame() {
    init();

    block_on(async {
        let loader = glycin_core::Loader::new_vec(instruction(&[b"panic-next-step"]));
        let mut image = loader.load().await.unwrap();
        let err = image.next_frame().await.unwrap_err();
        assert!(err.is_panic(), "Error: {err}");
    });
}

#[test]
fn glycin_test_panic_create() {
    init();

    block_on(async {
        let mut creator = glycin_core::Creator::new(MimeType::new_static("image/x-glycin-test"))
            .await
            .unwrap();

        let inst = instruction(&[b"panic"]);

        creator
            .add_frame(inst.len() as u32, 1, glycin_core::MemoryFormat::G8, inst)
            .unwrap();

        let err = creator.create().await.unwrap_err();

        assert!(err.is_panic(), "Error: {err}");
    });
}

#[test]
fn glycin_test_panic_edit() {
    init();

    block_on(async {
        let editor = glycin_core::Editor::new_vec(instruction(&[b"panic"]));

        let err = editor.edit().await.unwrap_err();

        assert!(err.is_panic(), "Error: {err}");
    });
}

#[test]
fn glycin_test_panic_apply_complete() {
    init();

    block_on(async {
        let editor = glycin_core::Editor::new_vec(instruction(&[b"panic-next-step"]));

        let editable_image = editor.edit().await.unwrap();

        let err = editable_image
            .apply_complete(&Operations::new(vec![Operation::MirrorHorizontally]))
            .await
            .unwrap_err();

        assert!(err.is_panic(), "Error: {err}");
    });
}

#[test]
fn glycin_test_panic_apply_sparse() {
    init();

    block_on(async {
        let editor = glycin_core::Editor::new_vec(instruction(&[b"panic-next-step"]));

        let editable_image = editor.edit().await.unwrap();

        let err = editable_image
            .apply_sparse(&Operations::new(vec![Operation::MirrorHorizontally]))
            .await
            .unwrap_err();

        assert!(err.is_panic(), "Error: {err}");
    });
}

#[test]
fn glycin_test_timeout_load() {
    init();

    block_on(async {
        let mut loader = glycin_core::Loader::new_vec(instruction(&[b"infinte-loop"]));
        loader.limits(Limits::default().timeout(Duration::from_millis(10)));

        let err = loader.load().await.unwrap_err();
        assert!(err.is_timeout(), "Error: {err}");
    });
}

#[test]
fn glycin_test_timeout_next_frame() {
    init();

    block_on(async {
        let mut loader = glycin_core::Loader::new_vec(instruction(&[b"infinte-loop-next-step"]));
        loader.limits(Limits::default().timeout(Duration::from_millis(100)));

        let mut image = loader.load().await.unwrap();

        let err = image.next_frame().await.unwrap_err();
        assert!(err.is_timeout(), "Error: {err}");
    });
}
