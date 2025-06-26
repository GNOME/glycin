use std::sync::Mutex;

use gio::glib;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glycin_utils::MemoryFormat;

use crate::NewImage;

static_assertions::assert_impl_all!(GlyNewImage: Send, Sync);
use std::collections::BTreeMap;

use super::init;

pub mod imp {
    use super::*;

    #[derive(Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyNewImage)]
    pub struct GlyNewImage {
        #[property(get, set, construct)]
        width: Mutex<u32>,
        #[property(get, set, construct)]
        height: Mutex<u32>,
        #[property(get, set, construct, builder(MemoryFormat::R8g8b8a8))]
        memory_format: Mutex<MemoryFormat>,
        #[property(get, set, construct)]
        texture: Mutex<Option<glib::Bytes>>,

        pub(super) key_value: Mutex<BTreeMap<String, String>>,
    }

    impl Default for GlyNewImage {
        fn default() -> Self {
            Self {
                height: Default::default(),
                width: Default::default(),
                memory_format: Mutex::new(MemoryFormat::R8g8b8a8),
                texture: Default::default(),
                key_value: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyNewImage {
        const NAME: &'static str = "GlyNewImage";
        type Type = super::GlyNewImage;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyNewImage {
        fn constructed(&self) {
            self.parent_constructed();

            init();

            let obj = self.obj();

            if obj.width() == 0 || obj.height() == 0 {
                glib::g_critical!(
                    "glycin",
                    "A GlyCreator needs to be initiatilized with width > 0 and height > 0."
                );
            }

            if obj.texture().is_none() {
                glib::g_critical!(
                    "glycin",
                    "A GlyCreator needs to be initiatilized with data."
                );
            }
        }
    }
}

glib::wrapper! {
    /// GObject wrapper for [`Loader`]
    pub struct GlyNewImage(ObjectSubclass<imp::GlyNewImage>);
}

impl GlyNewImage {
    pub fn new(width: u32, height: u32, memory_format: MemoryFormat, texture: glib::Bytes) -> Self {
        glib::Object::builder()
            .property("width", width)
            .property("height", height)
            .property("memory-format", memory_format)
            .property("texture", texture)
            .build()
    }

    pub fn new_image(&self) -> Result<NewImage, crate::Error> {
        let imp = self.imp();

        let mut new_image = NewImage::new(
            self.width(),
            self.height(),
            self.memory_format(),
            self.texture().unwrap(),
        )?;

        new_image.inner.image_info.key_value = Some(imp.key_value.lock().unwrap().clone());
        Ok(new_image)
    }

    pub fn add_key_value(&self, key: String, value: String) {
        self.imp().key_value.lock().unwrap().insert(key, value);
    }
}
