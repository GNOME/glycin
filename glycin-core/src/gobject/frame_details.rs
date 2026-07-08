use std::marker::PhantomData;
use std::sync::OnceLock;

use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::translate::*;

use crate::FrameDetails;
use crate::gobject::new_frame::GlyPhysicalDimensionUnit;

pub mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::GlyFrameDetails)]
    pub struct GlyFrameDetails {
        pub(super) frame_details: OnceLock<FrameDetails>,

        #[property(get=Self::get_pixel_density_x)]
        pixel_density_x: PhantomData<f64>,

        #[property(get=Self::get_pixel_density_x_unit, nullable, builder(GlyPhysicalDimensionUnit::default()))]
        pixel_density_x_unit: PhantomData<GlyPhysicalDimensionUnit>,

        #[property(get=Self::get_pixel_density_y)]
        pixel_density_y: PhantomData<f64>,

        #[property(get=Self::get_pixel_density_y_unit, nullable, builder(GlyPhysicalDimensionUnit::default()))]
        pixel_density_y_unit: PhantomData<GlyPhysicalDimensionUnit>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyFrameDetails {
        const NAME: &'static str = "GlyFrameDetails";
        type Type = super::GlyFrameDetails;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyFrameDetails {}

    impl GlyFrameDetails {
        fn get_pixel_density_x(&self) -> f64 {
            self.frame_details
                .get()
                .unwrap()
                .pixel_density()
                .map(|x| x.x().value())
                .unwrap_or_default()
        }

        fn get_pixel_density_x_unit(&self) -> GlyPhysicalDimensionUnit {
            self.frame_details
                .get()
                .unwrap()
                .pixel_density()
                .map(|x| unsafe { GlyPhysicalDimensionUnit::from_glib(x.x().unit().into()) })
                .unwrap_or_default()
        }

        fn get_pixel_density_y(&self) -> f64 {
            self.frame_details
                .get()
                .unwrap()
                .pixel_density()
                .map(|x| x.y().value())
                .unwrap_or_default()
        }

        fn get_pixel_density_y_unit(&self) -> GlyPhysicalDimensionUnit {
            self.frame_details
                .get()
                .unwrap()
                .pixel_density()
                .map(|x| unsafe { GlyPhysicalDimensionUnit::from_glib(x.y().unit().into()) })
                .unwrap_or_default()
        }
    }
}

glib::wrapper! {
    pub struct GlyFrameDetails(ObjectSubclass<imp::GlyFrameDetails>);
}

impl GlyFrameDetails {
    pub fn new(frame_details: FrameDetails) -> Self {
        let obj = glib::Object::new::<Self>();
        obj.imp().frame_details.set(frame_details).unwrap();
        obj
    }
}
