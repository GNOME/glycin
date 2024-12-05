use glycin::MemoryFormat;
use utils::*;

mod utils;

#[test]
fn change_memory_format() {
    block_on(change_memory_format_internal());
}

async fn change_memory_format_internal() {
    for path in [
        "test-images/images/color/color.png",
        "test-images/images/gray-iccp/gray-iccp-GA16.png",
    ] {
        let file = gio::File::for_path(path);

        for memory_format in [
            MemoryFormat::A8b8g8r8,
            MemoryFormat::R8g8b8,
            MemoryFormat::R16g16b16,
        ] {
            let mut loader = glycin::Loader::new(file.clone());
            loader.transform_to_memory_format(Some(memory_format));

            let image = loader.load().await.unwrap();

            image.next_frame().await.unwrap();
        }
    }
}
