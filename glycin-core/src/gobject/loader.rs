use std::marker::PhantomData;

use futures_util::lock::Mutex;
use gio::glib;
use glib::g_critical;
use glib::prelude::*;
use glib::subclass::prelude::*;
use glycin_common::MemoryFormatSelection;

use super::{GlyImage, init};
use crate::main_context::ProvidesMainContext;
use crate::{Loader, SandboxSelector};

static_assertions::assert_impl_all!(GlyLoader: Send, Sync);

pub mod imp {

    use super::*;

    #[derive(Default, Debug, glib::Properties)]
    #[properties(wrapper_type = super::GlyLoader)]
    pub struct GlyLoader {
        #[property(construct_only, set=Self::set_file)]
        file: PhantomData<gio::File>,
        #[property(construct_only, set=Self::set_stream, type=gio::InputStream)]
        stream: PhantomData<()>,
        #[property(construct_only, set=Self::set_bytes)]
        bytes: PhantomData<glib::Bytes>,

        #[property(get=Self::cancellable, set=Self::set_cancellable)]
        cancellable: PhantomData<gio::Cancellable>,
        #[property(set=Self::set_sandbox_selector, builder(SandboxSelector::default()))]
        sandbox_selector: PhantomData<SandboxSelector>,
        #[property(set=Self::set_accepted_memory_formats)]
        accepted_memory_formats: PhantomData<MemoryFormatSelection>,
        #[property(set=Self::set_apply_transformations)]
        apply_transformations: PhantomData<bool>,

        pub(super) loader: Mutex<Option<Loader>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlyLoader {
        const NAME: &'static str = "GlyLoader";
        type Type = super::GlyLoader;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GlyLoader {
        fn constructed(&self) {
            self.parent_constructed();

            init();

            if self.loader.try_lock().unwrap().is_none() {
                g_critical!(
                    "glycin",
                    "A loader needs to be initialized with exactly one of the 'file', 'stream', or 'bytes' properties. None specified."
                );
            }
        }
    }

    impl GlyLoader {
        fn set_bytes(&self, bytes: Option<glib::Bytes>) {
            let Some(bytes) = bytes else { return };

            self.init(Loader::new_bytes(bytes));
        }

        fn set_file(&self, file: Option<gio::File>) {
            let Some(file) = file else { return };

            self.init(Loader::new(file));
        }

        fn set_stream(&self, stream: Option<gio::InputStream>) {
            let Some(stream) = stream else { return };

            self.init(unsafe { Loader::new_stream(stream) });
        }

        fn init(&self, loader: Loader) {
            glib::MainContext::new().block_on(async {
                let mut loader_mutex =self.loader.lock().await;
               if loader_mutex.is_some() {
                        g_critical!(
                            "glycin",
                            "A loader needs to be initialized with exactly one of the 'file', 'stream', or 'bytes' properties. More than one specified."
                        );
                } else {
                  *loader_mutex = Some(loader);
                }
            })
        }

        fn cancellable(&self) -> gio::Cancellable {
            self.inspect(|x| x.cancellable.clone())
        }

        fn set_cancellable(&self, cancellable: gio::Cancellable) {
            self.inspect(|x| {
                x.cancellable(cancellable);
            });
        }

        fn set_sandbox_selector(&self, sandbox_selector: SandboxSelector) {
            self.inspect(|x| {
                x.sandbox_selector(sandbox_selector);
            });
        }

        fn set_accepted_memory_formats(&self, memory_format_selection: MemoryFormatSelection) {
            self.inspect(|x| {
                x.accepted_memory_formats(memory_format_selection);
            });
        }

        fn set_apply_transformations(&self, apply_transformations: bool) {
            self.inspect(|x| {
                x.apply_transformations(apply_transformations);
            });
        }

        pub(super) fn inspect<T: Default>(&self, f: impl FnOnce(&mut Loader) -> T) -> T {
            let Some(mut loader_lock) = self.loader.try_lock() else {
                g_critical!(
                    "glycin",
                    "Loader used from more than one thread at the same time."
                );
                return T::default();
            };

            let Some(loader) = loader_lock.as_mut() else {
                g_critical!(
                    "glycin",
                    "Loader used after Loader.load() or a similar function has been called."
                );
                return T::default();
            };

            f(loader)
        }
    }
}

glib::wrapper! {
    /// GObject wrapper for [`Loader`]
    pub struct GlyLoader(ObjectSubclass<imp::GlyLoader>);
}

impl GlyLoader {
    pub fn new(file: gio::File) -> Self {
        glib::Object::builder().property("file", file).build()
    }

    pub fn for_stream(stream: &gio::InputStream) -> Self {
        glib::Object::builder().property("stream", stream).build()
    }
    pub fn for_bytes(bytes: &glib::Bytes) -> Self {
        glib::Object::builder().property("bytes", bytes).build()
    }

    pub fn load(&self) -> Result<GlyImage, crate::Error> {
        glib::MainContext::new().block_on(async {
            let Some(mut loader) = std::mem::take(&mut *self.imp().loader.lock().await) else {
                return Err(crate::ErrorKind::LoaderUsedTwice.into());
            };

            loader.main_context_selector(crate::MainContextSelector::Managed);
            let image = loader.load().await?;

            Ok(GlyImage::new(image))
        })
    }

    pub async fn load_future(&self) -> Result<GlyImage, crate::Error> {
        let loader = std::mem::take(&mut *self.imp().loader.lock().await);
        let image = loader.unwrap().load().await?;
        Ok(GlyImage::new(image))
    }

    pub fn main_context(&self) -> glib::MainContext {
        self.imp().inspect(|x| x.main_context())
    }

    pub fn source_display(&self) -> String {
        self.imp().inspect(|x: &mut Loader| x.source.display())
    }
}
