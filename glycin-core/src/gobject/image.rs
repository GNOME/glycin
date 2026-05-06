use std::sync::OnceLock;

use futures_util::lock::{MappedMutexGuard, Mutex, MutexGuard};
use gio::{Cancellable, glib};
use glib::subclass::prelude::*;

use super::GlyFrame;
use crate::main_context::ProvidesMainContext;
use crate::{Error, FrameRequest, Image, ImageDetails, MainContextSelector};

static_assertions::assert_impl_all!(GlyImage: Send, Sync);

pub mod imp {

    use super::*;

    #[derive(Default, Debug)]
    pub struct GlyImage {
        pub(super) image: Mutex<Option<Image>>,
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
        dbg!("new");
        let obj = glib::Object::new::<Self>();
        dbg!("set");
        *obj.imp().image.try_lock().unwrap() = Some(image);
        dbg!("done");
        obj
    }

    pub fn image_info(&self) -> ImageDetails {
        self.image().details()
    }

    pub fn specific_frame(&self, frame_request: FrameRequest) -> Result<GlyFrame, Error> {
        glib::MainContext::new().block_on(async {
            let mut image = self.image();

            let mut main_context = MainContextSelector::Managed;
            std::mem::swap(&mut main_context, &mut image.loader.main_context_selector);

            let frame = image.specific_frame(frame_request).await?;

            image.loader.main_context_selector = main_context;

            Ok(GlyFrame::new(frame))
        })
    }

    pub async fn specific_frame_future(
        &self,
        frame_request: FrameRequest,
    ) -> Result<GlyFrame, Error> {
        Ok(GlyFrame::new(
            self.image().specific_frame(frame_request).await?,
        ))
    }

    pub fn cancellable(&self) -> Cancellable {
        self.image().cancellable()
    }

    pub fn image(&self) -> MappedMutexGuard<'_, Option<Image>, Image> {
        MutexGuard::map(
            self.imp()
                .image
                .try_lock()
                .expect("Image may not be used from two threads at the same time."),
            |x| x.as_mut().unwrap(),
        )
    }

    pub fn mime_type(&self) -> &glib::GString {
        self.imp()
            .mime_type
            .get_or_init(|| glib::GString::from(self.image().mime_type().as_str()))
    }

    pub fn main_context(&self) -> glib::MainContext {
        self.image().loader.main_context()
    }
}
