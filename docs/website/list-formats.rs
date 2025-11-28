#!/usr/bin/env -S cargo +nightly -Zscript
---
[dependencies]
glycin = { path = "../../glycin", features = ["unstable-config"] }
glib = "0.21"
gio = "0.21"
async-io = "2.5"
serde = { version = "1.0", features = ["derive"] }
serde_yaml_ng = "0.10"
itertools = "0.14"
---

use glycin::OperationId;
use itertools::Itertools;
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
#[serde(rename_all = "kebab-case")]
struct Details {
    exif: Option<String>,
    icc: Option<String>,
    cicp: Option<String>,
    xmp: Option<String>,
    animation: Option<String>,
    #[serde(default)]
    loader_codec: Codecs,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum Codecs {
    Codecs(Vec<Codec>),
    Codec(Codec),
}

impl Default for Codecs {
    fn default() -> Self {
        Self::Codecs(Vec::new())
    }
}

impl Codecs {
    fn to_vec(&self) -> Vec<Codec> {
        match self {
            Self::Codec(c) => vec![c.to_owned()],
            Self::Codecs(v) => v.to_vec(),
        }
    }

    fn html(&self) -> String {
        self.to_vec().into_iter().map(|x| x.html()).join(", ")
    }

    fn markdown(&self) -> String {
        self.to_vec().into_iter().map(|x| x.markdown()).join(", ")
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
enum Codec {
    Crate(String),
    CodecDetails(CodecDetails),
}

impl Codec {
    fn html(&self) -> String {
        match self {
            Self::Crate(cr) => {
                format!(", codec: <a href='https://crates.io/crates/{cr}'>{cr}</a> (Rust)")
            }
            Self::CodecDetails(CodecDetails { name, url, lang }) => {
                format!(", codec: <a href='{url}'>{name}</a> ({lang})")
            }
        }
    }

    fn markdown(&self) -> String {
        match self {
            Self::Crate(cr) => {
                format!("[{cr}](https://crates.io/crates/{cr}) (Rust)")
            }
            Self::CodecDetails(CodecDetails { name, url, lang }) => {
                format!("[{name}]({url}) ({lang})")
            }
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct CodecDetails {
    name: String,
    url: String,
    lang: String,
}

fn main() {
    let info = info();

    match std::env::args().nth(1).unwrap().as_str() {
        "html" => {
            println!("{}", html(info));
        }
        "markdown" => {
            println!("{}", markdown(info));
        }
        format => panic!("Unknown output format: {format}"),
    }
}

fn info() -> BTreeMap<String, Format> {
    let mut info = BTreeMap::<String, Format>::new();

    let details: BTreeMap<String, Details> =
        serde_yaml_ng::from_reader(std::fs::File::open("docs/website/format-details.yml").unwrap())
            .unwrap();

    dbg!(&details);

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
                let entry = info.entry(mime_type.to_string()).or_default();

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

    info
}

fn html(info: BTreeMap<String, Format>) -> String {
    let mut html = String::new();
    let s = &mut html;
    for (mime_type, info) in info {
        let ext = if let Some(ext) = glycin::MimeType::new(mime_type.clone()).extension() {
            format!(" (.{ext})")
        } else {
            String::new()
        };
        s.push_str(&format!("<h3>{} – {mime_type}{ext}</h3>", info.description));

        s.push_str(&format!(
            "<h4>Loader: {}{}</h4>",
            info.loader.unwrap().name,
            info.details.loader_codec.html()
        ));

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

    html
}

fn markdown(info: BTreeMap<String, Format>) -> String {
    let mut markdown = String::new();
    let s = &mut markdown;

    s.push_str("| Format | Glycin Loader | Decoder |\n");
    s.push_str("|-|-|-|\n");

    for (mime_type, info) in info {
        let ext = if let Some(ext) = glycin::MimeType::new(mime_type.clone()).extension() {
            format!(" (.{ext})")
        } else {
            String::new()
        };

        s.push_str("| ");

        s.push_str(&format!("{} {ext} |", info.description));

        s.push_str(&format!(" {} |", info.loader.unwrap().name,));

        s.push_str(&format!("{} |", info.details.loader_codec.markdown()));

        s.push_str("\n");
    }

    markdown
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
