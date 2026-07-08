use std::sync::OnceLock;

use gio::glib;
use glib::subclass::prelude::*;

use crate::Frame;
use crate::gobject::GlyFrameDetails;

static_assertions::assert_impl_all!(GlyFrame: Send, Sync);

#[derive(Debug, Copy, Clone, gio::glib::Enum)]
#[enum_type(name = "GlyColorMode")]
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

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyFrame)]
    pub struct GlyFrame {
        pub(super) frame: OnceLock<Frame>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyFrame {
        const NAME: &'static str = "GlyFrame";
        type Type = super::GlyFrame;
    }

    #[glib::derived_properties]
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

    pub fn details(&self) -> GlyFrameDetails {
        GlyFrameDetails::new(self.frame().details())
    }
}
