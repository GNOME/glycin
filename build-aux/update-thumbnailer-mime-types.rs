#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"

[dependencies]
glycin = { path = "../glycin" }
glib = "0.21"
async-io = "2.5"
---

// Rewrite the mime types in thumbnailer configs based on loader configs

fn main() {
    for entry in std::fs::read_dir("glycin-loaders").unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            let mut loader_path = entry.path();
            loader_path.push(entry.path().file_name().unwrap());
            loader_path.set_extension("conf");

            let mut thumbnailer_path = entry.path();
            thumbnailer_path.push(entry.path().file_name().unwrap());
            thumbnailer_path.set_extension("thumbnailer.in");

            let mut config = glycin::config::Config::default();
            async_io::block_on(glycin::config::Config::load_from_into(
                glycin::config::ConfigSource::File(loader_path.clone()),
                &mut config,
            ))
            .unwrap();

            let mut mime_types = config
                .loaders()
                .keys()
                .map(|x| x.to_string())
                .collect::<Vec<_>>();
            mime_types.sort();
            let mut mime_types = mime_types.join(";");
            mime_types.push(';');

            eprintln!("Mime types for {loader_path:?}:\n  {mime_types}\n");

            let thumbnailer_config = glib::KeyFile::new();

            thumbnailer_config
                .load_from_file(
                    &thumbnailer_path,
                    glib::KeyFileFlags::KEEP_COMMENTS | glib::KeyFileFlags::KEEP_TRANSLATIONS,
                )
                .unwrap();

            thumbnailer_config.set_string("Thumbnailer Entry", "MimeType", &mime_types);
            thumbnailer_config.save_to_file(thumbnailer_path).unwrap();
        }
    }
}
