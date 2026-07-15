fn main() {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    match (target_os.as_str(), target_env.as_str()) {
        // Set soname of library
        ("linux", _) => {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libglycin-gtk4-{major}.so.0")
        }
        ("macos", _) => println!(
            "cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libglycin-gtk4-{major}.0.dylib"
        ),
        ("windows", "msvc") => {
            // The import library (*.lib) always points at whatever /OUT:
            // filename the DLL was linked with, so the final installed
            // name has to be given to Cargo directly here
            let out_dir = std::env::var("OUT_DIR").unwrap();
            let profile_dir = std::path::Path::new(&out_dir)
                .ancestors()
                .nth(3)
                .expect("OUT_DIR does not have the expected Cargo layout");
            println!(
                "cargo:rustc-cdylib-link-arg=/OUT:{}",
                profile_dir
                    .join(format!("glycin-gtk4-{major}-0.dll"))
                    .display()
            );
            println!(
                "cargo:rustc-cdylib-link-arg=/IMPLIB:{}",
                profile_dir
                    .join(format!("glycin-gtk4-{major}.lib"))
                    .display()
            );
        }
        ("windows", "gnu") => {}
        _ => {}
    }

    system_deps::Config::new().probe().unwrap();
}
