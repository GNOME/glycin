use crate::MemoryFormat;

pub mod clip;
pub mod orientation;

pub struct SimpleFrame {
    pub width: u32,
    pub height: u32,
    /// Line stride
    pub stride: u32,
    pub memory_format: MemoryFormat,
}
