pub fn main() {
    async_io::block_on(async {
        dbg!(glycin::config::Config::cached().await);
        let loader = glycin::Loader::new(gio::File::for_path("bak/testout.png"));
        let image = loader.load().await.unwrap();
        dbg!(&image);
        let frame = image.next_frame().await.unwrap();
        dbg!(frame);
    });
}
