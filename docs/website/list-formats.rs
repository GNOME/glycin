#!/usr/bin/env -S cargo +nightly -Zscript
---
[dependencies]
glycin = { path = "../../glycin", features = ["unstable-config"] }
glib = "0.21"
gio = "0.21"
async-io = "2.5"
serde = { version = "1.0", features = ["derive"] }
serde_yaml_ng = { version = "0.10" }
---

use glycin::OperationId;
use std::collections::BTreeMap;

#[derive(Debug)]
struct Loader {
    name: String,
    config: glycin::config::ImageLoaderConfig,
}

#[derive(Debug)]
struct Editor {
    name: String,
    config: glycin::config::ImageEditorConfig,
}

#[derive(Debug, Default)]
struct Format {
    mime_type: String,
    description: String,
    details: Details,
    loader: Option<Loader>,
    editor: Option<Editor>,
}

#[derive(Debug, Default, Clone, serde::Deserialize)]
struct Details {
    exif: Option<String>,
    icc: Option<String>,
    cicp: Option<String>,
    xmp: Option<String>,
    animation: Option<String>,
}

fn main() {
    let mut info = BTreeMap::<String, Format>::new();

    let details: BTreeMap<String, Details> =
        serde_yaml_ng::from_reader(std::fs::File::open("docs/website/format-details.yml").unwrap())
            .unwrap();

    for entry in std::fs::read_dir("glycin-loaders").unwrap() {
        let entry = entry.unwrap();
        if !entry.path().is_dir() {
            continue;
        }

        for entry in std::fs::read_dir(entry.path()).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension() != Some(std::ffi::OsStr::new("conf")) {
                continue;
            }

            let name = path
                .parent()
                .unwrap()
                .file_name()
                .unwrap()
                .display()
                .to_string();
            eprintln!("{name}");

            let key_file = glib::KeyFile::new();
            key_file
                .load_from_file(&path, glib::KeyFileFlags::NONE)
                .unwrap();

            // Iterate over groups
            for group in key_file.groups() {
                let mut group = group.split(':');
                let type_ = group.next().unwrap();
                let mime_type = glycin::MimeType::new(group.next().unwrap().to_string());
                eprintln!("{type_}: {mime_type}");

                let mut config = glycin::config::Config::default();
                async_io::block_on(glycin::config::Config::load_file(&path, &mut config)).unwrap();
                let mut entry = info.entry(mime_type.to_string()).or_default();

                entry.mime_type = mime_type.to_string();
                entry.description =
                    gio::content_type_get_description(&mime_type.to_string()).to_string();
                entry.details = details
                    .get(&mime_type.to_string())
                    .map(|x| x.clone())
                    .unwrap_or_default();

                match type_ {
                    "loader" => {
                        entry.loader = Some(Loader {
                            name: name.clone(),
                            config: config.loader(&mime_type).unwrap().clone(),
                        });
                    }
                    "editor" => {
                        entry.editor = Some(Editor {
                            name: name.clone(),
                            config: config.editor(&mime_type).unwrap().clone(),
                        });
                    }
                    _ => {
                        unreachable!()
                    }
                }
            }
        }
    }

    let s = &mut String::new();
    for (mime_type, info) in info {
        let ext = if let Some(ext) = glycin::MimeType::new(mime_type.clone()).extension() {
            format!(" (.{ext})")
        } else {
            String::new()
        };
        s.push_str(&format!("<h3>{} – {mime_type}{ext}</h3>", info.description));

        s.push_str(&format!("<h4>Loader: {}</h4>", info.loader.unwrap().name));

        s.push_str("<ul class='features'>");
        add_flag(s, "ICC Profile", info.details.icc);
        add_flag(s, "CICP", info.details.cicp);
        add_flag(s, "Exif", info.details.exif);
        add_flag(s, "XMP", info.details.xmp);
        add_flag(s, "Animation", info.details.animation);
        s.push_str("</ul>");

        if let Some(editor) = info.editor {
            s.push_str(&format!("<h4>Editor: {}</h4>", &editor.name));

            s.push_str("<ul class='features'>");
            add_flag(s, "Create Images", Some(editor.config.creator.to_string()));

            for (operation, name) in [(OperationId::Clip, "Clip"), (OperationId::Rotate, "Rotate")]
            {
                if editor.config.operations.contains(&operation) {
                    s.push_str(&format!("<li class='implemented' title='The editing feature “{name}” is implemented for this format.'>✔ {name}</li>"))
                }
            }

            s.push_str("</ul>");
        }
    }
    print!("{s}");
}

fn add_entry(s: &mut String, name: &str, value: &str) {
    s.push_str(&format!("{name}: {value}\n"))
}

fn add_flag(s: &mut String, name: &str, v: Option<String>) {
    match v.as_deref() {
        Some("true") => s.push_str(&format!("<li class='implemented' title='The feature “{name}” is implemented for this format.'>✔ {name}</li>")),
        Some("false") => s.push_str(&format!("<li class='missing' title='The feature “{name}” is not yet implemented for this format.'>🗙 {name}</li>")),
        Some("unsupported") => {}
        None => s.push_str(&format!("<li class='unknown' title='It is unknown if the format supports the feature “{name}”.'>🯄 {name}</li>")),
        Some(x) => panic!("Unsupported value: {x}"),
    }
}
