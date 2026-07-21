mod indentifier;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
#[cfg(feature = "external")]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use futures_util::StreamExt;
use gio::glib;
use glycin_common::{MemoryFormat, OperationId};

use crate::config::indentifier::Identifier;
use crate::util::{self, AsyncMutex, new_async_mutex, read};
use crate::{Error, ErrorKind, SandboxMechanism};

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
        Some(self.cmp(other))
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

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub(crate) image_loader: BTreeMap<MimeType, ImageLoaderConfig>,
    pub(crate) image_editor: BTreeMap<MimeType, ImageEditorConfig>,
}

impl Config {
    pub fn loaders(&self) -> &BTreeMap<MimeType, ImageLoaderConfig> {
        &self.image_loader
    }

    pub fn editors(&self) -> &BTreeMap<MimeType, ImageEditorConfig> {
        &self.image_editor
    }

    pub(crate) fn guess_mime_type(
        &self,
        path: Option<&Path>,
        head: &[u8],
        editor: bool,
    ) -> Option<MimeType> {
        let config: Box<dyn Iterator<Item = (&MimeType, ConfigEntry)>> = if editor {
            Box::new(
                self.image_editor
                    .iter()
                    .map(|(k, v)| (k, ConfigEntry::Editor(v.clone()))),
            )
        } else {
            Box::new(
                self.image_loader
                    .iter()
                    .map(|(k, v)| (k, ConfigEntry::Loader(v.clone()))),
            )
        };

        let mut complexities = config
            .flat_map(|(_, x)| {
                x.identifiers()
                    .iter()
                    .map(|x| x.complexity())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        complexities.sort();

        for complexity in complexities.into_iter().rev() {
            let find = self.image_loader.iter().find(|(_, x)| {
                x.identifiers
                    .iter()
                    .any(|x| x.complexity() == complexity && x.matches(path, head))
            });

            if let Some((mime_type, _)) = find {
                return Some(mime_type.clone());
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
pub enum ConfigEntry {
    Editor(ImageEditorConfig),
    Loader(ImageLoaderConfig),
}

#[derive(Debug, Clone)]
pub struct ImageLoaderConfig {
    pub processor: Processor,
    pub identifiers: Vec<Identifier>,
    pub expose_base_dir: bool,
    pub fontconfig: bool,
}

#[derive(Debug, Clone)]
pub enum Processor {
    #[cfg(feature = "external")]
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
        Some(self.cmp(other))
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
            #[cfg(feature = "external")]
            Self::Binary(path) => Some(path.as_path()),
            #[cfg(feature = "builtin")]
            Self::Builtin(_) => None,
        }
    }

    pub fn hash(&self) -> &[u8] {
        match self {
            #[cfg(feature = "external")]
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
    pub(crate) processor: Processor,
    pub(crate) identifiers: Vec<Identifier>,
    pub(crate) expose_base_dir: bool,
    pub(crate) fontconfig: bool,
    pub(crate) operations: BTreeSet<OperationId>,
    pub(crate) creator: bool,
    pub(crate) creator_color_icc_profile: bool,
    pub(crate) creator_encoding_quality: bool,
    pub(crate) creator_encoding_compression: bool,
    pub(crate) creator_metadata_key_value: bool,
    pub(crate) creator_pixel_density: bool,
    pub(crate) creator_memory_formats: BTreeSet<MemoryFormat>,
}

impl ImageEditorConfig {
    /// Memory formats which the creator supports for writing
    pub fn creator_memory_formats(&self) -> &BTreeSet<MemoryFormat> {
        &self.creator_memory_formats
    }

    /// Supported editing operations
    pub fn operations(&self) -> &BTreeSet<OperationId> {
        &self.operations
    }

    pub fn is_creator(&self) -> bool {
        self.creator
    }
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

    pub fn identifiers(&self) -> &[Identifier] {
        match self {
            Self::Editor(e) => &e.identifiers,
            Self::Loader(l) => &l.identifiers,
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
            return Err(ErrorKind::NoLoadersConfigured(self.clone()).err());
        }

        self.image_loader
            .get(mime_type)
            .ok_or_else(|| ErrorKind::UnknownImageFormat(mime_type.to_string(), self.clone()).err())
    }

    pub fn editor(&self, mime_type: &MimeType) -> Result<&ImageEditorConfig, Error> {
        self.image_editor
            .get(mime_type)
            .ok_or_else(|| ErrorKind::UnknownImageFormat(mime_type.to_string(), self.clone()).err())
    }

    async fn load() -> Self {
        let mut config = Config::default();

        #[cfg(feature = "builtin-image-rs")]
        Self::load_builtin_config(
            BuiltinProcessor::ImageRs(glycin_image_rs::BuiltinImageRs),
            &mut config,
        )
        .await;

        #[cfg(feature = "builtin-test")]
        Self::load_builtin_config(
            BuiltinProcessor::Test(glycin_test::BuiltinTest),
            &mut config,
        )
        .await;

        #[cfg(feature = "external")]
        for mut data_dir in Self::data_dirs() {
            data_dir.push("glycin-loaders");
            data_dir.push(format!("{}+", crate::COMPAT_VERSION));
            data_dir.push("conf.d");

            if let Ok(mut config_files) = util::read_dir(data_dir).await {
                while let Some(result) = config_files.next().await {
                    if let Ok(path) = result
                        && path.extension() == Some(OsStr::new(CONFIG_FILE_EXT))
                        && let Err(err) =
                            Self::load_config(ConfigProcessor::File(path.clone()), &mut config)
                                .await
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
        if let Err(err) = Self::load_config(ConfigProcessor::Builtin(builtin), config).await {
            tracing::error!("Failed to load builtin config for '{name}': {err}");
        }
    }

    pub async fn load_config(
        loader: ConfigProcessor,
        config: &mut Config,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let data = match &loader {
            #[cfg(feature = "external")]
            ConfigProcessor::File(path) => {
                tracing::trace!("Loading config file {path:?}");
                read(path).await?
            }
            #[cfg(feature = "builtin")]
            ConfigProcessor::Builtin(builtin) => builtin.common().config().as_bytes().to_vec(),
        };

        let bytes = glib::Bytes::from_owned(data);

        let keyfile = glib::KeyFile::new();
        keyfile.load_from_bytes(&bytes, glib::KeyFileFlags::NONE)?;

        let mut loader_mime_types = Vec::new();
        let mut editor_mime_types = Vec::new();

        for group in keyfile.groups() {
            let warning = "Unknown config group: {group}. Expected [loader:<mime-type>] or [editor:<mime-type>]. Ignoring.";

            let elements = group.trim().split_once(':');

            let Some((kind, mime_type)) = elements else {
                tracing::warn!("{warning}");
                continue;
            };

            let entry = (group.to_string(), MimeType::new(mime_type.to_string()));

            match kind {
                "loader" => loader_mime_types.push(entry),
                "editor" => editor_mime_types.push(entry),
                _ => tracing::warn!("{warning}"),
            }
        }

        for (group, mime_type) in loader_mime_types {
            if config.image_loader.contains_key(&mime_type) {
                continue;
            }

            let exec = keyfile.string(&group, "Exec")?;

            let processor = match loader {
                #[cfg(feature = "external")]
                ConfigProcessor::File(_) => Processor::Binary(exec.into()),
                #[cfg(feature = "builtin")]
                ConfigProcessor::Builtin(ref builtin) => Processor::Builtin(builtin.clone()),
            };

            let identifiers = Self::load_identifiers(&keyfile, &group)?.unwrap_or_default();

            let expose_base_dir =
                Self::handle_and_default(keyfile.boolean(&group, "ExposeBaseDir"))?;
            let fontconfig = Self::handle_and_default(keyfile.boolean(&group, "Fontconfig"))?;

            let cfg = ImageLoaderConfig {
                processor,
                expose_base_dir,
                fontconfig,
                identifiers,
            };

            config.image_loader.insert(mime_type, cfg);
        }

        for (group, mime_type) in editor_mime_types {
            if config.image_editor.contains_key(&mime_type) {
                continue;
            }

            let equiv_loader = config.image_loader.get(&mime_type);

            let exec = match keyfile.string(&group, "Exec") {
                Ok(x) => x.into(),
                Err(err) => {
                    if err.matches(glib::KeyFileError::KeyNotFound) {
                        // Try to use previously defined loader Exec, otherwise, return editor's
                        // original error
                        equiv_loader
                            .and_then(|x| x.processor.exec().map(|x| x.to_path_buf()))
                            .ok_or(err)?
                    } else {
                        return Err(Box::new(err));
                    }
                }
            };

            let processor = match loader {
                #[cfg(feature = "external")]
                ConfigProcessor::File(_) => Processor::Binary(exec),
                #[cfg(feature = "builtin")]
                ConfigProcessor::Builtin(ref builtin) => Processor::Builtin(builtin.clone()),
            };

            // Use identifiers previously defined in a loader with the same mime type, if
            // not defined in editor
            let identifiers = Self::load_identifiers(&keyfile, &group)?
                .or_else(|| equiv_loader.map(|x| x.identifiers.clone()))
                .unwrap_or_default();

            let expose_base_dir = keyfile.boolean(&group, "ExposeBaseDir").unwrap_or_default();
            let fontconfig = keyfile.boolean(&group, "Fontconfig").unwrap_or_default();

            let operations_str = keyfile
                .string_list(&group, "Operations")
                .unwrap_or_default();
            let operations = BTreeSet::from_iter(
                operations_str
                    .into_iter()
                    .flat_map(|x| OperationId::from_str(&x)),
            );

            let creator = Self::handle_and_default(keyfile.boolean(&group, "Creator"))?;

            let creator_color_icc_profile =
                Self::handle_and_default(keyfile.boolean(&group, "CreatorColorIccProfile"))?;

            let creator_encoding_compression =
                Self::handle_and_default(keyfile.boolean(&group, "CreatorEncodingCompression"))?;

            let creator_encoding_quality =
                Self::handle_and_default(keyfile.boolean(&group, "CreatorEncodingQuality"))?;

            let creator_metadata_key_value =
                Self::handle_and_default(keyfile.boolean(&group, "CreatorMetadataKeyValue"))?;

            let creator_pixel_density =
                Self::handle_and_default(keyfile.boolean(&group, "CreatorPixelDensity"))?;

            let creator_memory_formats = BTreeSet::from_iter(
                keyfile
                    .string_list(&group, "CreatorMemoryFormats")
                    .unwrap_or_default()
                    .into_iter()
                    .flat_map(|x| {
                        let f = MemoryFormat::from_str(&x);
                        if f.is_none() {
                            tracing::warn!("Unknown memory format '{x}' found in {mime_type}")
                        }
                        f
                    }),
            );

            let cfg = ImageEditorConfig {
                processor,
                identifiers,
                expose_base_dir,
                fontconfig,
                operations,
                creator,
                creator_color_icc_profile,
                creator_encoding_compression,
                creator_encoding_quality,
                creator_metadata_key_value,
                creator_pixel_density,
                creator_memory_formats,
            };

            config.image_editor.insert(mime_type, cfg);
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

    fn handle_and_default<T: Default>(res: Result<T, glib::Error>) -> Result<T, glib::Error> {
        Self::handle(res).map(|x| x.unwrap_or_default())
    }

    fn handle<T>(res: Result<T, glib::Error>) -> Result<Option<T>, glib::Error> {
        match res {
            Err(err) => {
                if err.matches(glib::KeyFileError::KeyNotFound) {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
            Ok(x) => Ok(Some(x)),
        }
    }

    fn load_identifiers(
        keyfile: &glib::KeyFile,
        group: &str,
    ) -> Result<Option<Vec<Identifier>>, glib::Error> {
        let Some(itentifiers) = Self::handle(keyfile.string_list(group, "Identifiers"))? else {
            return Ok(None);
        };

        Ok(Some(
            itentifiers
                .iter()
                .filter_map(|x| match Identifier::parse(x) {
                    Err(err) => {
                        tracing::warn!("{group}: Invalid identifier: {err}");
                        None
                    }
                    Ok(x) => Some(x),
                })
                .collect(),
        ))
    }
}

pub enum ConfigProcessor {
    #[cfg(feature = "external")]
    File(PathBuf),
    #[cfg(feature = "builtin")]
    Builtin(BuiltinProcessor),
}

#[cfg(feature = "builtin")]
#[derive(Debug, Clone)]
pub enum BuiltinProcessor {
    #[cfg(feature = "builtin-image-rs")]
    ImageRs(glycin_image_rs::BuiltinImageRs),
    #[cfg(feature = "builtin-test")]
    Test(glycin_test::BuiltinTest),
}

#[cfg(feature = "builtin")]
impl BuiltinProcessor {
    pub fn common(&self) -> &dyn glycin_utils::Builtin {
        match self {
            #[cfg(feature = "builtin-image-rs")]
            Self::ImageRs(processor) => processor,
            #[cfg(feature = "builtin-test")]
            Self::Test(processor) => processor,
        }
    }
}
