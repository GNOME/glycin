use std::sync::OnceLock;

use gio::{glib, Cancellable};
use glib::subclass::prelude::*;

use super::GlyFrame;
use crate::{ErrorCtx, FrameRequest, Image, ImageDetails};

static_assertions::assert_impl_all!(GlyImage: Send, Sync);

pub mod imp {
    use super::*;

    #[derive(Default, Debug)]
    pub struct GlyImage {
        pub(super) image: OnceLock<Image>,
        pub(super) mime_type: OnceLock<glib::GString>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyImage {
        const NAME: &'static str = "GlyImage";
        type Type = super::GlyImage;
    }

    impl ObjectImpl for GlyImage {}
}

glib::wrapper! {
    /// GObject wrapper for [`Image`]
    pub struct GlyImage(ObjectSubclass<imp::GlyImage>);
}

impl GlyImage {
    pub(crate) fn new(image: Image) -> Self {
        let obj = glib::Object::new::<Self>();
        obj.imp().image.set(image).unwrap();
        obj
    }

    pub fn image_info(&self) -> ImageDetails {
        self.image().details()
    }

    pub async fn next_frame(&self) -> Result<GlyFrame, ErrorCtx> {
        Ok(GlyFrame::new(self.image().next_frame().await?))
    }

    pub async fn specific_frame(&self, frame_request: FrameRequest) -> Result<GlyFrame, ErrorCtx> {
        Ok(GlyFrame::new(
            self.image().specific_frame(frame_request).await?,
        ))
    }

    pub fn cancellable(&self) -> Cancellable {
        self.image().cancellable()
    }

    pub fn image(&self) -> &Image {
        self.imp().image.get().unwrap()
    }

    pub fn mime_type(&self) -> &glib::GString {
        self.imp().mime_type.get_or_init(|| {
            glib::GString::from(self.imp().image.get().unwrap().mime_type().as_str())
        })
    }
}
