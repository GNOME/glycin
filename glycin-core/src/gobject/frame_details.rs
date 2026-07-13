use std::marker::PhantomData;
use std::sync::OnceLock;

use glib::prelude::*;
use glib::subclass::prelude::*;
use glib::translate::*;

use crate::FrameDetails;
use crate::gobject::pixel_density::GlyPixelDensity;

pub mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::GlyFrameDetails)]
    pub struct GlyFrameDetails {
        pub(super) frame_details: OnceLock<FrameDetails>,

        #[property(get=Self::pixel_density, nullable)]
        pixel_density: PhantomData<Option<GlyPixelDensity>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyFrameDetails {
        const NAME: &'static str = "GlyFrameDetails";
        type Type = super::GlyFrameDetails;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyFrameDetails {}

    impl GlyFrameDetails {
        fn pixel_density(&self) -> Option<GlyPixelDensity> {
            self.frame_details
                .get()
                .unwrap()
                .pixel_density()
                .map(|x| GlyPixelDensity::new(x))
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
