use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use futures_util::StreamExt;
use gio::glib;
use glycin_common::OperationId;

use crate::util::{AsyncMutex, new_async_mutex, read, read_dir};
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
    pub processor: Processor,
    pub expose_base_dir: bool,
    pub fontconfig: bool,
}

#[derive(Debug, Clone)]
pub enum Processor {
    Binary(PathBuf),
    #[cfg(feature = "builtin")]
    Builtin(BuiltinProcessor),
}

impl PartialEq for Processor {
    fn eq(&self, other: &Self) -> bool {
        self.hash().eq(other.hash())
    }
}

impl Eq for Processor {}

impl PartialOrd for Processor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.hash().cmp(other.hash()))
    }
}

impl Ord for Processor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hash().cmp(other.hash())
    }
}

impl Processor {
    pub fn exec(&self) -> Option<&Path> {
        match self {
            Self::Binary(path) => Some(path.as_path()),
            #[cfg(feature = "builtin")]
            Self::Builtin(_) => None,
        }
    }

    pub fn hash(&self) -> &[u8] {
        match self {
            Self::Binary(path) => path.as_os_str().as_bytes(),
            #[cfg(feature = "builtin")]
            Self::Builtin(builtin) => builtin.common().name().as_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfigEntryHash {
    fontconfig: bool,
    processor: Processor,
    expose_base_dir: bool,
    base_dir: Option<PathBuf>,
    sandbox_mechanism: SandboxMechanism,
}

impl ConfigEntryHash {
    pub fn exec(&self) -> Option<&Path> {
        self.processor.exec()
    }
}

#[derive(Debug, Clone)]
pub struct ImageEditorConfig {
    pub processor: Processor,
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
            processor: self.processor().clone(),
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

    pub fn exec(&self) -> Option<&Path> {
        match self {
            Self::Editor(e) => e.processor.exec(),
            Self::Loader(l) => l.processor.exec(),
        }
    }

    pub fn processor(&self) -> &Processor {
        match self {
            Self::Editor(e) => &e.processor,
            Self::Loader(l) => &l.processor,
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

        #[cfg(feature = "builtin-image-rs")]
        Self::load_builtin_config(
            BuiltinProcessor::ImageRs(glycin_image_rs::BuiltinImageRs),
            &mut config,
        )
        .await;

        for mut data_dir in Self::data_dirs() {
            data_dir.push("glycin-loaders");
            data_dir.push(format!("{COMPAT_VERSION}+"));
            data_dir.push("conf.d");

            if let Ok(mut config_files) = read_dir(data_dir).await {
                while let Some(result) = config_files.next().await {
                    if let Ok(path) = result
                        && path.extension() == Some(OsStr::new(CONFIG_FILE_EXT))
                        && let Err(err) =
                            Self::load_config(ConfigLoader::File(path.clone()), &mut config).await
                    {
                        tracing::error!("Failed to load config file {path:?}: {err}");
                    }
                }
            }
        }

        config
    }

    #[cfg(feature = "builtin")]
    pub async fn load_builtin_config(builtin: BuiltinProcessor, config: &mut Config) {
        let name = builtin.common().name();
        if let Err(err) = Self::load_config(ConfigLoader::Builtin(builtin), config).await {
            tracing::error!("Failed to load builtin config for '{name}': {err}");
        }
    }

    pub async fn load_config(
        loader: ConfigLoader,
        config: &mut Config,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = match &loader {
            ConfigLoader::File(path) => {
                tracing::trace!("Loading config file {path:?}");
                read(path).await?
            }
            #[cfg(feature = "builtin")]
            ConfigLoader::Builtin(builtin) => builtin.common().config().as_bytes().to_vec(),
        };

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
                            let processor = match loader {
                                ConfigLoader::File(_) => Processor::Binary(exec.into()),
                                #[cfg(feature = "builtin")]
                                ConfigLoader::Builtin(ref builtin) => {
                                    Processor::Builtin(builtin.clone())
                                }
                            };

                            let expose_base_dir =
                                keyfile.boolean(group, "ExposeBaseDir").unwrap_or_default();
                            let fontconfig =
                                keyfile.boolean(group, "Fontconfig").unwrap_or_default();

                            let cfg = ImageLoaderConfig {
                                processor,
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
                            let processor = match loader {
                                ConfigLoader::File(_) => Processor::Binary(exec.into()),
                                #[cfg(feature = "builtin")]
                                ConfigLoader::Builtin(ref builtin) => {
                                    Processor::Builtin(builtin.clone())
                                }
                            };

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
                                processor,
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

pub enum ConfigLoader {
    File(PathBuf),
    #[cfg(feature = "builtin")]
    Builtin(BuiltinProcessor),
}

#[cfg(feature = "builtin")]
#[derive(Debug, Clone)]
pub enum BuiltinProcessor {
    #[cfg(feature = "builtin-image-rs")]
    ImageRs(glycin_image_rs::BuiltinImageRs),
}

#[cfg(feature = "builtin")]
impl BuiltinProcessor {
    fn common(&self) -> &dyn glycin_utils::Builtin {
        match self {
            #[cfg(feature = "builtin-image-rs")]
            Self::ImageRs(processor) => processor,
        }
    }
}
