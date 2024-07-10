fn main() {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    // Set soname of library
    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libglycin-gtk4-{major}.so.0");

    system_deps::Config::new().probe().unwrap();
}
