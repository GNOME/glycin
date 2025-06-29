use std::marker::PhantomData;
use std::sync::OnceLock;

use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;

use crate::EncodedImage;

static_assertions::assert_impl_all!(GlyEncodedImage: Send, Sync);

pub mod imp {
    use super::*;

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyEncodedImage)]
    pub struct GlyEncodedImage {
        #[property(get=Self::data, nullable)]
        data: PhantomData<Option<glib::Bytes>>,

        pub(super) encoded_image: OnceLock<EncodedImage>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyEncodedImage {
        const NAME: &'static str = "GlyEncodedImage";
        type Type = super::GlyEncodedImage;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyEncodedImage {}

    impl GlyEncodedImage {
        fn data(&self) -> Option<glib::Bytes> {
            self.encoded_image
                .get()
                .unwrap()
                .data_full()
                .map_err(|err| {
                    glib::g_warning!(
                        "glycin",
                        "Internal error when retrieving encoded image data: {err}"
                    );
                    err
                })
                .ok()
                .map(glib::Bytes::from_owned)
        }
    }
}

glib::wrapper! {
    /// GObject wrapper for [`Loader`]
    pub struct GlyEncodedImage(ObjectSubclass<imp::GlyEncodedImage>);
}

impl GlyEncodedImage {
    pub fn new(encoded_image: EncodedImage) -> Self {
        let obj = glib::Object::new::<Self>();
        obj.imp().encoded_image.set(encoded_image).unwrap();
        obj
    }
}
