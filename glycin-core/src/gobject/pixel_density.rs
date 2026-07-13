use std::marker::PhantomData;
use std::sync::MutexGuard;

use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::translate::*;
use gufo::common::physical_dimension;
use gufo_common::physical_dimension::PixelsPerPhysicalDimension;

use crate::gobject::GlyPhysicalDimensionUnit;

pub mod imp {

    use std::sync::Mutex;

    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::GlyPixelDensity)]
    pub struct GlyPixelDensity {
        pub(super) pixel_density: Mutex<Option<physical_dimension::PixelDensity>>,

        #[property(get=Self::x_value, set=Self::set_x_value)]
        x_value: PhantomData<f64>,

        #[property(get=Self::x_unit, set=Self::set_x_unit, builder(GlyPhysicalDimensionUnit::default()))]
        x_unit: PhantomData<GlyPhysicalDimensionUnit>,

        #[property(get=Self::y_value, set=Self::set_y_value)]
        y_value: PhantomData<f64>,

        #[property(get=Self::y_unit, set=Self::set_y_unit, builder(GlyPhysicalDimensionUnit::default()))]
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
            self.obj().inner().as_ref().unwrap().x().value()
        }

        fn set_x_value(&self, value: f64) {
            let x = self.obj().inner().as_ref().unwrap().x();
            let y = self.obj().inner().as_ref().unwrap().y();
            *self.obj().inner() = Some(physical_dimension::PixelDensity::new(
                physical_dimension::PixelsPerPhysicalDimension::new(value, x.unit()),
                y,
            ))
        }

        fn x_unit(&self) -> GlyPhysicalDimensionUnit {
            unsafe {
                GlyPhysicalDimensionUnit::from_glib(
                    self.obj().inner().as_ref().unwrap().x().unit().into(),
                )
            }
        }

        fn set_x_unit(&self, unit: GlyPhysicalDimensionUnit) {
            let unit = physical_dimension::PhysicalDimensionUnit::try_from(unit as i32).unwrap();
            let x = self.obj().inner().as_ref().unwrap().x();
            let y = self.obj().inner().as_ref().unwrap().y();
            *self.obj().inner() = Some(physical_dimension::PixelDensity::new(
                physical_dimension::PixelsPerPhysicalDimension::new(x.value(), unit),
                y,
            ))
        }

        fn y_value(&self) -> f64 {
            self.obj().inner().as_ref().unwrap().y().value()
        }

        fn set_y_value(&self, value: f64) {
            let x = self.obj().inner().as_ref().unwrap().x();
            let y = self.obj().inner().as_ref().unwrap().y();
            *self.obj().inner() = Some(physical_dimension::PixelDensity::new(
                x,
                physical_dimension::PixelsPerPhysicalDimension::new(value, y.unit()),
            ))
        }

        fn y_unit(&self) -> GlyPhysicalDimensionUnit {
            unsafe {
                GlyPhysicalDimensionUnit::from_glib(
                    self.obj().inner().as_ref().unwrap().y().unit().into(),
                )
            }
        }

        fn set_y_unit(&self, unit: GlyPhysicalDimensionUnit) {
            let unit = physical_dimension::PhysicalDimensionUnit::try_from(unit as i32).unwrap();
            let x = self.obj().inner().as_ref().unwrap().x();
            let y = self.obj().inner().as_ref().unwrap().y();
            *self.obj().inner() = Some(physical_dimension::PixelDensity::new(
                x,
                physical_dimension::PixelsPerPhysicalDimension::new(y.value(), unit),
            ))
        }
    }
}

glib::wrapper! {
    pub struct GlyPixelDensity(ObjectSubclass<imp::GlyPixelDensity>);
}

impl GlyPixelDensity {
    pub fn new(pixel_density: physical_dimension::PixelDensity) -> Self {
        glib::Object::builder()
            .property("x-value", pixel_density.x().value())
            .property("x-unit", unsafe {
                GlyPhysicalDimensionUnit::from_glib(pixel_density.x().unit() as i32)
            })
            .property("y-value", pixel_density.y().value())
            .property("y-unit", unsafe {
                GlyPhysicalDimensionUnit::from_glib(pixel_density.y().unit() as i32)
            })
            .build()
    }

    pub fn for_values(
        x_value: f64,
        x_unit: GlyPhysicalDimensionUnit,
        y_value: f64,
        y_unit: GlyPhysicalDimensionUnit,
    ) -> Self {
        let x_unit = physical_dimension::PhysicalDimensionUnit::try_from(x_unit as i32).unwrap();
        let y_unit = physical_dimension::PhysicalDimensionUnit::try_from(y_unit as i32).unwrap();

        let pixel_density = physical_dimension::PixelDensity::new(
            physical_dimension::PixelsPerPhysicalDimension::new(x_value, x_unit),
            physical_dimension::PixelsPerPhysicalDimension::new(y_value, y_unit),
        );

        let obj = glib::Object::new::<Self>();
        *obj.inner() = Some(pixel_density);
        obj
    }

    pub(crate) fn inner(&self) -> MutexGuard<'_, Option<physical_dimension::PixelDensity>> {
        let mut lock = self.imp().pixel_density.lock().unwrap();

        if lock.is_none() {
            *lock = Some(physical_dimension::PixelDensity::new(
                PixelsPerPhysicalDimension::new(
                    0.,
                    physical_dimension::PhysicalDimensionUnit::Inch,
                ),
                PixelsPerPhysicalDimension::new(
                    0.,
                    physical_dimension::PhysicalDimensionUnit::Inch,
                ),
            ));
        }

        lock
    }

    pub fn convert(&self, unit: GlyPhysicalDimensionUnit) -> Self {
        let unit = physical_dimension::PhysicalDimensionUnit::try_from(unit as i32).unwrap();
        Self::new(self.inner().as_ref().unwrap().convert(unit))
    }
}
