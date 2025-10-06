use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use futures_util::StreamExt;
use gio::glib;
use glycin_common::OperationId;

use crate::util::{new_async_mutex, read, read_dir, AsyncMutex};
use crate::{Error, SandboxMechanism};

#[derive(Clone, Debug)]
/// Mime type
pub enum MimeType {
    Alloc(String),
    Stack(&'static str),
}

impl PartialEq for MimeType {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for MimeType {}

impl PartialOrd for MimeType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl Ord for MimeType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl MimeType {
    pub const BMP: Self = Self::new_static("image/bmp");
    /// No encoding
    pub const DDS: Self = Self::new_static("image/x-dds");
    pub const GIF: Self = Self::new_static("image/gif");
    pub const ICO: Self = Self::new_static("image/vnd.microsoft.icon");
    pub const JPEG: Self = Self::new_static("image/jpeg");
    pub const OPEN_EXR: Self = Self::new_static("image/x-exr");
    pub const PNG: Self = Self::new_static("image/png");
    pub const QOI: Self = Self::new_static("image/qoi");
    pub const TGA: Self = Self::new_static("image/x-tga");
    pub const TIFF: Self = Self::new_static("image/tiff");
    pub const WEBP: Self = Self::new_static("image/webp");

    pub const AVIF: Self = Self::new_static("image/avif");
    pub const HEIC: Self = Self::new_static("image/heif");

    pub const JXL: Self = Self::new_static("image/jxl");

    const EXTENSIONS: &[(Self, &'static str)] = &[
        (Self::AVIF, "avif"),
        (Self::BMP, "bmp"),
        (Self::DDS, "dds"),
        (Self::GIF, "gif"),
        (Self::HEIC, "heic"),
        (Self::ICO, "ico"),
        (Self::JPEG, "jpg"),
        (Self::JXL, "jxl"),
        (Self::OPEN_EXR, "exr"),
        (Self::PNG, "png"),
        (Self::QOI, "qoi"),
        (Self::TGA, "tga"),
        (Self::TIFF, "tiff"),
        (Self::WEBP, "webp"),
    ];

    pub fn new(mime_type: String) -> Self {
        Self::Alloc(mime_type)
    }

    pub const fn new_static(mime_type: &'static str) -> Self {
        Self::Stack(mime_type)
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Alloc(s) => s.as_str(),
            Self::Stack(str) => str,
        }
    }

    /// File extension
    pub fn extension(&self) -> Option<&'static str> {
        Self::EXTENSIONS
            .iter()
            .find(|x| x.0.as_str() == self.as_str())
            .map(|x| x.1)
    }
}

impl From<&str> for MimeType {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

impl std::fmt::Display for MimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

const CONFIG_FILE_EXT: &str = "conf";
pub const COMPAT_VERSION: u8 = 2;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub(crate) image_loader: BTreeMap<MimeType, ImageLoaderConfig>,
    pub(crate) image_editor: BTreeMap<MimeType, ImageEditorConfig>,
}

#[derive(Debug, Clone)]
pub enum ConfigEntry {
    Editor(ImageEditorConfig),
    Loader(ImageLoaderConfig),
}

#[derive(Debug, Clone)]
pub struct ImageLoaderConfig {
    pub exec: PathBuf,
    pub expose_base_dir: bool,
    pub fontconfig: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfigEntryHash {
    fontconfig: bool,
    exec: PathBuf,
    expose_base_dir: bool,
    base_dir: Option<PathBuf>,
    sandbox_mechanism: SandboxMechanism,
}

impl ConfigEntryHash {
    pub fn exec(&self) -> &Path {
        &self.exec
    }
}

#[derive(Debug, Clone)]
pub struct ImageEditorConfig {
    pub exec: PathBuf,
    pub expose_base_dir: bool,
    pub fontconfig: bool,
    pub operations: Vec<OperationId>,
    pub creator: bool,
    pub creator_color_icc_profile: bool,
    pub creator_encoding_quality: bool,
    pub creator_encoding_compression: bool,
    pub creator_metadata_key_value: bool,
}

impl ConfigEntry {
    pub fn hash_value(
        &self,
        base_dir: Option<PathBuf>,
        sandbox_mechanism: SandboxMechanism,
    ) -> ConfigEntryHash {
        ConfigEntryHash {
            fontconfig: self.fontconfig(),
            exec: self.exec().to_owned(),
            expose_base_dir: self.expose_base_dir(),
            base_dir,
            sandbox_mechanism,
        }
    }

    pub fn fontconfig(&self) -> bool {
        match self {
            Self::Editor(e) => e.fontconfig,
            Self::Loader(l) => l.fontconfig,
        }
    }

    pub fn exec(&self) -> &Path {
        match self {
            Self::Editor(e) => &e.exec,
            Self::Loader(l) => &l.exec,
        }
    }

    pub fn expose_base_dir(&self) -> bool {
        match self {
            Self::Editor(e) => e.expose_base_dir,
            Self::Loader(l) => l.expose_base_dir,
        }
    }
}

impl Config {
    pub async fn cached() -> Arc<Self> {
        static CONFIG: AsyncMutex<Option<Arc<Config>>> = new_async_mutex(None);
        let mut config = CONFIG.lock().await;

        if let Some(config) = config.clone() {
            config
        } else {
            let loaded_config = Arc::new(Self::load().await);
            *config = Some(loaded_config.clone());
            loaded_config
        }
    }

    pub fn loader(&self, mime_type: &MimeType) -> Result<&ImageLoaderConfig, Error> {
        if self.image_loader.is_empty() {
            return Err(Error::NoLoadersConfigured(self.clone()));
        }

        self.image_loader
            .get(mime_type)
            .ok_or_else(|| Error::UnknownImageFormat(mime_type.to_string(), self.clone()))
    }

    pub fn editor(&self, mime_type: &MimeType) -> Result<&ImageEditorConfig, Error> {
        self.image_editor
            .get(mime_type)
            .ok_or_else(|| Error::UnknownImageFormat(mime_type.to_string(), self.clone()))
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
                                tracing::error!("Failed to load config file: {err}");
                            }
                        }
                    }
                }
            }
        }

        config
    }

    pub async fn load_file(
        path: &Path,
        config: &mut Config,
    ) -> Result<(), Box<dyn std::error::Error>> {
        tracing::trace!("Loading config file {path:?}");

        let data = read(path).await?;
        let bytes = glib::Bytes::from_owned(data);

        let keyfile = glib::KeyFile::new();
        keyfile.load_from_bytes(&bytes, glib::KeyFileFlags::NONE)?;

        for group in keyfile.groups() {
            let mut elements = group.trim().split(':');
            let kind = elements.next();
            let mime_type = elements.next();

            if let Some(mime_type) = mime_type {
                let mime_type = MimeType::new(mime_type.to_string());
                let group = group.trim();
                match kind {
                    Some("loader") => {
                        if config.image_loader.contains_key(&mime_type) {
                            continue;
                        }

                        if let Ok(exec) = keyfile.string(group, "Exec") {
                            let expose_base_dir =
                                keyfile.boolean(group, "ExposeBaseDir").unwrap_or_default();
                            let fontconfig =
                                keyfile.boolean(group, "Fontconfig").unwrap_or_default();

                            let cfg = ImageLoaderConfig {
                                exec: exec.into(),
                                expose_base_dir,
                                fontconfig,
                            };

                            config.image_loader.insert(mime_type, cfg);
                        }
                    }
                    Some("editor") => {
                        if config.image_editor.contains_key(&mime_type) {
                            continue;
                        }

                        if let Ok(exec) = keyfile.string(group, "Exec") {
                            let expose_base_dir =
                                keyfile.boolean(group, "ExposeBaseDir").unwrap_or_default();
                            let fontconfig =
                                keyfile.boolean(group, "Fontconfig").unwrap_or_default();

                            let operations_str =
                                keyfile.string_list(group, "Operations").unwrap_or_default();
                            let operations = operations_str
                                .into_iter()
                                .flat_map(|x| OperationId::from_str(&x))
                                .collect();

                            let creator = keyfile.boolean(group, "Creator").unwrap_or_default();

                            let creator_color_icc_profile = keyfile
                                .boolean(group, "CreatorColorIccProfile")
                                .unwrap_or_default();

                            let creator_encoding_compression = keyfile
                                .boolean(group, "CreatorEncodingCompression")
                                .unwrap_or_default();

                            let creator_encoding_quality = keyfile
                                .boolean(group, "CreatorEncodingQuality")
                                .unwrap_or_default();

                            let creator_metadata_key_value = keyfile
                                .boolean(group, "CreatorMetadataKeyValue")
                                .unwrap_or_default();

                            let cfg = ImageEditorConfig {
                                exec: exec.into(),
                                expose_base_dir,
                                fontconfig,
                                operations,
                                creator,
                                creator_color_icc_profile,
                                creator_encoding_compression,
                                creator_encoding_quality,
                                creator_metadata_key_value,
                            };

                            config.image_editor.insert(mime_type, cfg);
                        }
                    }
                    _ => {}
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
            let mut data_dirs = vec![glib::user_data_dir()];
            data_dirs.extend(glib::system_data_dirs());
            data_dirs
        }
    }
}
