use std::time::Duration;

#[cfg(feature = "external")]
use zbus::zvariant::{Type, as_value};

#[derive(Debug)]
#[cfg_attr(
    feature = "external",
    derive(serde::Deserialize, serde::Serialize, Type)
)]
#[cfg_attr(feature = "external", zvariant(signature = "dict"))]
#[cfg_attr(feature = "external", serde(default))]
#[non_exhaustive]
pub struct Limits {
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub max_dimensions: (u32, u32),
    #[cfg_attr(feature = "external", serde(with = "as_value"))]
    pub timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_dimensions: (u16::MAX as u32, u16::MAX as u32),
            timeout: Duration::from_secs(60),
        }
    }
}
