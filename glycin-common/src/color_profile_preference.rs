use serde::{Deserialize, Serialize};
use zvariant::Type;

#[repr(i32)]
#[derive(Deserialize, Serialize, Type, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "gobject", derive(glib::Enum))]
#[cfg_attr(feature = "gobject", enum_type(name = "GlyMemoryFormat"))]
#[zvariant(signature = "s")]
/// Some loaders can provide a CICP and ICC profile at the same time. Which
/// color information should be preferred differs between image formats. This
/// value determines which kind should be preferred.
pub enum ColorProfilePreference {
    #[default]
    Cicp,
    IccProfile,
}
