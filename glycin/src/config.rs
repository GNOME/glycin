use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use futures_util::StreamExt;
use gio::glib;

use crate::util::{read, read_dir};
use crate::Error;

pub type MimeType = String;

const CONFIG_FILE_EXT: &str = "conf";
pub const COMPAT_VERSION: u8 = 1;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub image_decoders: HashMap<MimeType, ImageDecoderConfig>,
}

#[derive(Debug, Clone)]
pub struct ImageDecoderConfig {
    pub exec: PathBuf,
    pub expose_base_dir: bool,
}

impl Config {
    pub async fn cached() -> &'static Self {
        if let Some(config) = CONFIG.get() {
            config
        } else {
            let config = Self::load().await;
            CONFIG.get_or_init(|| config)
        }
    }

    pub fn get(&self, mime_type: &MimeType) -> Result<&ImageDecoderConfig, Error> {
        self.image_decoders
            .get(mime_type.as_str())
            .ok_or_else(|| Error::UnknownImageFormat(mime_type.to_string()))
    }

    async fn load() -> Self {
        let mut config = Config::default();

        for mut data_dir in Self::data_dirs() {
            data_dir.push("glycin-loaders");
            data_dir.push(format!("{COMPAT_VERSION}+"));
            data_dir.push("conf.d");

            if let Ok(mut config_files) = read_dir(data_dir).await {
                while let Some(result) = config_files.next().await {
                    if let Ok(path) = result {
                        if path.extension() == Some(OsStr::new(CONFIG_FILE_EXT)) {
                            if let Err(err) = Self::load_file(&path, &mut config).await {
                                eprintln!("Failed to load config file: {err}");
                            }
                        }
                    }
                }
            }
        }

        config
    }

    async fn load_file(path: &Path, config: &mut Config) -> Result<(), Box<dyn std::error::Error>> {
        let data = read(path).await?;
        let bytes = glib::Bytes::from_owned(data);

        let keyfile = glib::KeyFile::new();
        keyfile.load_from_bytes(&bytes, glib::KeyFileFlags::NONE)?;

        for group in keyfile.groups() {
            let mut elements = group.split(':');
            let kind = elements.next();
            let mime_type = elements.next();

            if kind == Some("loader") {
                if let Some(mime_type) = mime_type {
                    let group = group.trim();
                    if let Ok(exec) = keyfile.string(group, "Exec") {
                        let expose_base_dir =
                            keyfile.boolean(group, "ExposeBaseDir").unwrap_or_default();

                        let cfg = ImageDecoderConfig {
                            exec: exec.into(),
                            expose_base_dir,
                        };

                        config.image_decoders.insert(mime_type.to_string(), cfg);
                    }
                }
            }
        }

        Ok(())
    }

    fn data_dirs() -> Vec<PathBuf> {
        // Force only specific data dir via env variable
        if let Some(data_dir) = std::env::var_os("GLYCIN_DATA_DIR") {
            vec![data_dir.into()]
        } else {
            let mut data_dirs = glib::system_data_dirs();
            data_dirs.push(glib::user_data_dir());
            data_dirs
        }
    }
}
