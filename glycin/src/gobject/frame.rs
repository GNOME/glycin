use std::sync::OnceLock;

use gio::glib;
use glib::subclass::prelude::*;

use crate::Frame;

static_assertions::assert_impl_all!(GlyFrame: Send, Sync);

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "gobject", derive(gio::glib::Enum))]
#[cfg_attr(feature = "gobject", enum_type(name = "GlyColorMode"))]
#[repr(i32)]
#[non_exhaustive]
pub enum GlyColorMode {
    Srgb,
    Cicp,
}

#[derive(Clone, Debug, glib::Boxed)]
#[boxed_type(name = "GlyCicp", nullable)]
#[repr(C)]
pub struct GlyCicp {
    pub color_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
    pub video_full_range_flag: u8,
}

pub mod imp {
    use super::*;

    #[derive(Default, Debug)]
    pub struct GlyFrame {
        pub(super) frame: OnceLock<Frame>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyFrame {
        const NAME: &'static str = "GlyFrame";
        type Type = super::GlyFrame;
    }

    impl ObjectImpl for GlyFrame {}
}

glib::wrapper! {
    /// GObject wrapper for [`Frame`]
    pub struct GlyFrame(ObjectSubclass<imp::GlyFrame>);
}

impl GlyFrame {
    pub(crate) fn new(frame: Frame) -> Self {
        let obj = glib::Object::new::<Self>();
        obj.imp().frame.set(frame).unwrap();
        obj
    }

    pub fn frame(&self) -> &Frame {
        self.imp().frame.get().unwrap()
    }

    pub fn color_mode(&self) -> GlyColorMode {
        match self.frame().color_state() {
            crate::ColorState::Srgb => GlyColorMode::Srgb,
            crate::ColorState::Cicp(_) => GlyColorMode::Cicp,
        }
    }

    pub fn color_cicp(&self) -> Option<crate::Cicp> {
        if let crate::ColorState::Cicp(cicp) = self.frame().color_state() {
            Some(*cicp)
        } else {
            None
        }
    }
}
