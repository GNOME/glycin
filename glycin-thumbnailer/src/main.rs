use gio::glib;

fn main() -> glib::ExitCode {
    glycin_thumbnailer::main(std::env::args().collect::<Vec<_>>())
}
