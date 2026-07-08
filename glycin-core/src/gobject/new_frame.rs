use std::sync::{Mutex, OnceLock};

use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glycin_utils::MemoryFormat;
use gufo_common::physical_dimension::{
    PhysicalDimensionUnit, PixelDensity, PixelsPerPhysicalDimension,
};

use super::init;

static_assertions::assert_impl_all!(GlyNewFrame: Send, Sync);

#[derive(Debug, Copy, Clone, gio::glib::Enum, Default)]
#[enum_type(name = "GlyPhysicalDimensionUnit")]
#[repr(i32)]
#[non_exhaustive]
pub enum GlyPhysicalDimensionUnit {
    #[default]
    Inch = 1,
    /// 1/6 inch
    Pica = 2,
    /// 1/72 inch
    Point = 3,
    Meter = 4,
    Centimeter = 5,
    Millimeter = 6,
}

pub mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::GlyNewFrame)]
    pub struct GlyNewFrame {
        #[property(get, construct_only)]
        width: OnceLock<u32>,
        #[property(get, construct_only)]
        height: OnceLock<u32>,
        #[property(get, construct_only)]
        stride: OnceLock<u32>,
        #[property(get, construct_only, builder(MemoryFormat::R8g8b8))]
        memory_format: OnceLock<MemoryFormat>,
        #[property(get, construct_only)]
        texture: OnceLock<glib::Bytes>,

        #[property(get, set, nullable)]
        color_icc_profile: Mutex<Option<glib::Bytes>>,

        pub(crate) pixel_density: Mutex<Option<PixelDensity>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyNewFrame {
        const NAME: &'static str = "GlyNewFrame";
        type Type = super::GlyNewFrame;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyNewFrame {
        fn constructed(&self) {
            self.parent_constructed();

            init();
        }
    }
}

glib::wrapper! {
    /// GObject wrapper for [`Loader`]
    pub struct GlyNewFrame(ObjectSubclass<imp::GlyNewFrame>);
}

impl GlyNewFrame {
    pub fn new(
        width: u32,
        height: u32,
        stride: Option<u32>,
        memory_format: MemoryFormat,
        texture: glib::Bytes,
    ) -> Self {
        glib::Object::builder()
            .property("width", width)
            .property("height", height)
            .property("stride", stride.unwrap_or_default())
            .property("memory-format", memory_format)
            .property("texture", texture)
            .build()
    }

    pub fn set_pixel_density(
        &self,
        pixel_density: Option<(f64, GlyPhysicalDimensionUnit, f64, GlyPhysicalDimensionUnit)>,
    ) {
        if let Some((x_value, x_unit, y_value, y_unit)) = pixel_density {
            let (Some(x_unit), Some(y_unit)) = (
                PhysicalDimensionUnit::try_from(x_unit as i32).ok(),
                PhysicalDimensionUnit::try_from(y_unit as i32).ok(),
            ) else {
                glib::g_critical!("glycin", "Invalid unit passed {y_unit:?} or {x_unit:?}");
                return;
            };

            let pixel_density = PixelDensity::new(
                PixelsPerPhysicalDimension::new(x_value, x_unit),
                PixelsPerPhysicalDimension::new(y_value, y_unit),
            );
            *self.imp().pixel_density.lock().unwrap() = Some(pixel_density);
        } else {
            *self.imp().pixel_density.lock().unwrap() = None;
        }
    }

    pub async fn build(&self, creator: &mut crate::Creator) -> Result<(), crate::Error> {
        let frame = if self.stride() == 0 {
            creator.add_frame(
                self.width(),
                self.height(),
                self.memory_format(),
                self.texture().into_data().to_vec(),
            )?
        } else {
            creator.add_frame_with_stride(
                self.width(),
                self.height(),
                self.stride(),
                self.memory_format(),
                self.texture().into_data().to_vec(),
            )?
        };

        frame.set_color_icc_profile(self.color_icc_profile().map(|x| x.into_data().to_vec()))?;

        Ok(())
    }
}
