use std::sync::LazyLock;

/// Specify which [`MainContext`](glib::MainContext) to use
#[derive(Debug, Default, Clone)]
pub enum MainContextSelector {
    /// Automatically detect which main context to use
    ///
    /// If a thread default is available, it is used. Otherwise, the global main
    /// context if it has a MainLoop. The [`Managed`](Self::Managed) option is
    /// used as fallback.
    #[default]
    Auto,
    /// Use a global shared loop exclusive to glycin operations. The thread,
    /// loop, and context is managed by glycin.
    Managed,
    /// Use the speicifed context.
    Specific(glib::MainContext),
}

static CHECK_MAIN_CONTEXT: LazyLock<std::sync::Mutex<()>> = LazyLock::new(Default::default);
pub(crate) trait ProvidesMainContext {
    fn main_context(&self) -> glib::MainContext;
}

impl ProvidesMainContext for crate::Loader {
    fn main_context(&self) -> glib::MainContext {
        main_context(&self.main_context_selector)
    }
}
impl ProvidesMainContext for crate::Editor {
    fn main_context(&self) -> glib::MainContext {
        main_context(&self.main_context_selector)
    }
}

fn main_context(selector: &MainContextSelector) -> glib::MainContext {
    let main_context = match selector {
        MainContextSelector::Auto => context_auto(),
        MainContextSelector::Managed => context_managed(),
        MainContextSelector::Specific(main_context) => main_context.clone(),
    };

    #[cfg(feature = "tokio")]
        main_context.spawn_from_within(|| async {
            if tokio::runtime::Handle::try_current().is_err() {
                tracing::error!("Using a MainContext which doesn't have a tokio Runtime in it's MainLoop thread. This will most likely fail.");
            }
        });

    main_context
}

fn context_auto() -> glib::MainContext {
    if let Some(thread_context) = glib::MainContext::thread_default() {
        tracing::debug!("Using current threads default MainContext.");
        // Current thread has a default MainContext
        thread_context
    } else {
        let check_main_context_lock = CHECK_MAIN_CONTEXT.lock().unwrap();
        let default_thread = glib::MainContext::default();
        let global_default_has_main_loop = default_thread.acquire().is_err();
        drop(check_main_context_lock);

        if global_default_has_main_loop {
            tracing::debug!("Using global default MainContext.");
            // Default thread is running on some other thread
            default_thread.clone()
        } else {
            context_managed()
        }
    }
}

fn context_managed() -> glib::MainContext {
    tracing::debug!("Using global glycin MainContext.");
    static GLYCIN_MAIN_CONTEXT: LazyLock<glib::MainContext> = LazyLock::new(|| {
        tracing::debug!("Creating glycin global MainContext.");

        let main_context = glib::MainContext::new();
        let main_loop = glib::MainLoop::new(Some(&main_context), true);

        #[cfg(feature = "tokio")]
        let hdl = tokio::runtime::Handle::current();

        std::thread::spawn(glib::clone!(
            #[strong]
            main_context,
            move || {
                // Inherit the tokio runtime for our custom thread
                #[cfg(feature = "tokio")]
                let _hdl = hdl.enter();
                main_context.with_thread_default(|| main_loop.run())
            }
        ));

        main_context
    });

    (*GLYCIN_MAIN_CONTEXT).clone()
}
