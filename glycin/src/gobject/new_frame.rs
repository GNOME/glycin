use std::sync::Arc;

use gio::glib;
use glib::subclass::prelude::*;

static_assertions::assert_impl_all!(GlyNewFrame: Send, Sync);

use super::init;
use crate::NewFrame;

pub mod imp {
    use std::sync::OnceLock;

    use super::*;

    #[derive(Debug, Default)]
    pub struct GlyNewFrame {
        pub(super) new_frame: OnceLock<Arc<NewFrame>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyNewFrame {
        const NAME: &'static str = "GlyNewFrame";
        type Type = super::GlyNewFrame;
    }

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
    pub fn new(new_frame: Arc<NewFrame>) -> Self {
        let obj = glib::Object::new::<Self>();

        obj.imp().new_frame.set(new_frame).unwrap();

        obj
    }

    pub fn new_frame(&self) -> Arc<NewFrame> {
        self.imp().new_frame.get().unwrap().clone()
    }
}
