#[derive(Debug, Copy, Clone, gio::glib::Enum)]
#[enum_type(name = "GlyColorMode")]
#[repr(i32)]
#[non_exhaustive]
pub enum GlyColorMode {
    Srgb = 1,
    Cicp = 2,
    IccProfile = 3,
}
