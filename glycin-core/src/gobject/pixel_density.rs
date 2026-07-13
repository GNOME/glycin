use std::marker::PhantomData;
use std::sync::OnceLock;

use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::translate::*;
use gufo::common::physical_dimension;

use crate::gobject::GlyPhysicalDimensionUnit;

pub mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::GlyPixelDensity)]
    pub struct GlyPixelDensity {
        pub(super) pixel_density: OnceLock<physical_dimension::PixelDensity>,

        #[property(get=Self::x_value)]
        x_value: PhantomData<f64>,

        #[property(get=Self::x_unit, nullable, builder(GlyPhysicalDimensionUnit::default()))]
        x_unit: PhantomData<GlyPhysicalDimensionUnit>,

        #[property(get=Self::y_value)]
        y_value: PhantomData<f64>,

        #[property(get=Self::y_unit, nullable, builder(GlyPhysicalDimensionUnit::default()))]
        y_unit: PhantomData<GlyPhysicalDimensionUnit>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyPixelDensity {
        const NAME: &'static str = "GlyPixelDensity";
        type Type = super::GlyPixelDensity;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyPixelDensity {}

    impl GlyPixelDensity {
        fn x_value(&self) -> f64 {
            self.pixel_density.get().unwrap().x().value()
        }

        fn x_unit(&self) -> GlyPhysicalDimensionUnit {
            unsafe {
                GlyPhysicalDimensionUnit::from_glib(
                    self.pixel_density.get().unwrap().x().unit().into(),
                )
            }
        }

        fn y_value(&self) -> f64 {
            self.pixel_density.get().unwrap().y().value()
        }

        fn y_unit(&self) -> GlyPhysicalDimensionUnit {
            unsafe {
                GlyPhysicalDimensionUnit::from_glib(
                    self.pixel_density.get().unwrap().y().unit().into(),
                )
            }
        }
    }
}

glib::wrapper! {
    pub struct GlyPixelDensity(ObjectSubclass<imp::GlyPixelDensity>);
}

impl GlyPixelDensity {
    pub fn new(pixel_density: physical_dimension::PixelDensity) -> Self {
        let obj = glib::Object::new::<Self>();
        obj.imp().pixel_density.set(pixel_density).unwrap();
        obj
    }

    pub fn convert(&self, unit: GlyPhysicalDimensionUnit) -> Self {
        let unit = physical_dimension::PhysicalDimensionUnit::try_from(unit as i32).unwrap();
        Self::new(self.imp().pixel_density.get().unwrap().convert(unit))
    }
}
