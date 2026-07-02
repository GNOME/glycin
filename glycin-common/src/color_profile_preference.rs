use serde::{Deserialize, Serialize};
use zvariant::Type;

#[repr(i32)]
#[derive(Deserialize, Serialize, Type, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "gobject", derive(glib::Enum))]
#[cfg_attr(feature = "gobject", enum_type(name = "GlyMemoryFormat"))]
#[zvariant(signature = "s")]
pub enum ColorProfilePreference {
    #[default]
    Cicp,
    IccProfile,
}
