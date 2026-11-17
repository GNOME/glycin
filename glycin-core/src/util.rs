use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use gio::glib;
use gio::prelude::CancellableExtManual;
#[cfg(feature = "gdk4")]
use glycin_utils::MemoryFormat;

use crate::error::ErrorKind;
#[cfg(feature = "external")]
use crate::sandbox::Sandbox;
#[cfg(feature = "gdk4")]
use crate::{ColorState, Error};

pub trait ShortcutErrorFuture<T, E>: Future<Output = Result<T, crate::Error>> + Sized
where
    E: Future<Output = Result<(), crate::Error>>,
{
    async fn join_abort_on_error(self, error_future: E) -> <Self as Future>::Output {
        let self_ = std::pin::pin!(self);
        let error_future = std::pin::pin!(error_future);
        let either = futures_util::future::select(error_future, self_).await;

        match either {
            futures_util::future::Either::Left((Err(err), _)) => Err(err),
            futures_util::future::Either::Left((Ok(_), f)) => f.await,
            futures_util::future::Either::Right((res, _)) => res,
        }
    }
}

impl<F, T, E> ShortcutErrorFuture<T, E> for F
where
    F: Future<Output = Result<T, crate::Error>> + Sized,
    E: Future<Output = Result<(), crate::Error>>,
{
}

pub trait CancellableFuture<T>: Future<Output = Result<T, crate::Error>> + Sized {
    async fn make_cancellable(self, cancellable: gio::Cancellable) -> <Self as Future>::Output {
        let self_ = std::pin::pin!(self);
        let either = futures_util::future::select(cancellable.future(), self_).await;
        match either {
            futures_util::future::Either::Left(_) => Err(ErrorKind::Canceled(None).err()),
            futures_util::future::Either::Right((res, _)) => res,
        }
    }
}

impl<T, F: Future<Output = Result<T, crate::Error>>> CancellableFuture<T> for F {}

pub trait TimeoutFuture<T>: Future<Output = Result<T, crate::Error>> + Sized {
    async fn enforce_timeout(self, timeout: Duration) -> <Self as Future>::Output {
        let self_ = std::pin::pin!(self);
        let timeout_ = std::pin::pin!(timeout_future(timeout));
        let either = futures_util::future::select(timeout_, self_).await;
        match either {
            futures_util::future::Either::Left(_) => Err(ErrorKind::Timeout(timeout).err()),
            futures_util::future::Either::Right((res, _)) => res,
        }
    }
}

impl<T, F: Future<Output = Result<T, crate::Error>>> TimeoutFuture<T> for F {}

#[cfg(feature = "gdk4")]
pub const fn gdk_memory_format(format: MemoryFormat) -> gdk::MemoryFormat {
    match format {
        MemoryFormat::B8g8r8a8Premultiplied => gdk::MemoryFormat::B8g8r8a8Premultiplied,
        MemoryFormat::A8r8g8b8Premultiplied => gdk::MemoryFormat::A8r8g8b8Premultiplied,
        MemoryFormat::R8g8b8a8Premultiplied => gdk::MemoryFormat::R8g8b8a8Premultiplied,
        MemoryFormat::B8g8r8a8 => gdk::MemoryFormat::B8g8r8a8,
        MemoryFormat::A8r8g8b8 => gdk::MemoryFormat::A8r8g8b8,
        MemoryFormat::R8g8b8a8 => gdk::MemoryFormat::R8g8b8a8,
        MemoryFormat::A8b8g8r8 => gdk::MemoryFormat::A8b8g8r8,
        MemoryFormat::R8g8b8 => gdk::MemoryFormat::R8g8b8,
        MemoryFormat::B8g8r8 => gdk::MemoryFormat::B8g8r8,
        MemoryFormat::R16g16b16 => gdk::MemoryFormat::R16g16b16,
        MemoryFormat::R16g16b16a16Premultiplied => gdk::MemoryFormat::R16g16b16a16Premultiplied,
        MemoryFormat::R16g16b16a16 => gdk::MemoryFormat::R16g16b16a16,
        MemoryFormat::R16g16b16Float => gdk::MemoryFormat::R16g16b16Float,
        MemoryFormat::R16g16b16a16Float => gdk::MemoryFormat::R16g16b16a16Float,
        MemoryFormat::R32g32b32Float => gdk::MemoryFormat::R32g32b32Float,
        MemoryFormat::R32g32b32a32FloatPremultiplied => {
            gdk::MemoryFormat::R32g32b32a32FloatPremultiplied
        }
        MemoryFormat::R32g32b32a32Float => gdk::MemoryFormat::R32g32b32a32Float,
        MemoryFormat::G8a8Premultiplied => gdk::MemoryFormat::G8a8Premultiplied,
        MemoryFormat::G8a8 => gdk::MemoryFormat::G8a8,
        MemoryFormat::G8 => gdk::MemoryFormat::G8,
        MemoryFormat::G16a16Premultiplied => gdk::MemoryFormat::G16a16Premultiplied,
        MemoryFormat::G16a16 => gdk::MemoryFormat::G16a16,
        MemoryFormat::G16 => gdk::MemoryFormat::G16,
        _ => panic!(),
    }
}

#[cfg(feature = "gdk4")]
pub fn gdk_color_state(format: &ColorState) -> Result<gdk::ColorState, crate::Error> {
    match format {
        ColorState::Srgb => Ok(gdk::ColorState::srgb()),
        ColorState::Cicp(cicp) => {
            use gufo_common::cicp::VideoRangeFlag;

            let cicp_params = gdk::CicpParams::new();

            cicp_params.set_color_primaries(u8::from(cicp.color_primaries).into());
            cicp_params.set_transfer_function(u8::from(cicp.transfer_characteristics).into());
            cicp_params.set_matrix_coefficients(u8::from(cicp.matrix_coefficients).into());

            let range = match cicp.video_full_range_flag {
                VideoRangeFlag::Full => gdk::CicpRange::Full,
                VideoRangeFlag::Narrow => gdk::CicpRange::Narrow,
            };
            cicp_params.set_range(range);

            Ok(cicp_params.build_color_state()?)
        }
        ColorState::IccProfile(_) => Err(Error::other(
            "GTK 4 doesn't support ICC profiles in color states yet. Set Loader::color_convert_icc_srgb to true to avoid this error.",
        )),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RunEnvironment {
    /// Sandbox force disabled
    SandboxForceDisabled,
    /// Not inside Flatpak
    Host,
    /// Not inside Flatpak but bwrap doesn't work
    HostBwrapSyscallsBlocked,
    /// Inside Flatpak
    Flatpak,
    /// Inside Flatpak and development environment
    FlatpakDevel,
}

impl RunEnvironment {
    #[cfg(feature = "external")]
    pub async fn cached() -> Self {
        static RUN_ENVIRONMENT: AsyncMutex<Option<RunEnvironment>> = new_async_mutex(None);

        let mut run_environment = RUN_ENVIRONMENT.lock().await;

        if let Some(result) = *run_environment {
            result
        } else {
            let run_env = if std::env::var("GLYCIN_DISABLE_SANDBOX").as_deref()
                == Ok("i-know-the-risks")
            {
                eprintln!(
                    "WARNING: Glycin running without sandbox. Force disabled via environment variable."
                );
                Self::SandboxForceDisabled
            } else if let Some(devel) = flatpak_devel().await {
                if devel {
                    eprintln!(
                        "WARNING: Glycin running without sandbox. Disabled due to Flatpak development environment."
                    );
                    Self::FlatpakDevel
                } else {
                    Self::Flatpak
                }
            } else if Sandbox::check_bwrap_syscalls_blocked().await {
                eprintln!(
                    "WARNING: Glycin running without sandbox. Bubblewrap (bwrap) doesn't work in the environment."
                );
                Self::HostBwrapSyscallsBlocked
            } else {
                Self::Host
            };

            *run_environment = Some(run_env);
            run_env
        }
    }

    #[cfg(not(feature = "external"))]
    pub async fn cached() -> Self {
        Self::Host
    }
}

/// Returns None if not in Flatpak environment, otherwise true if development
#[cfg(feature = "external")]
async fn flatpak_devel() -> Option<bool> {
    let data = read("/.flatpak-info").await.ok()?;
    let bytes = glib::Bytes::from_owned(data);

    let keyfile = glib::KeyFile::new();
    keyfile
        .load_from_bytes(&bytes, glib::KeyFileFlags::NONE)
        .ok()?;

    // App is not installed but instead started with `flatpak-builder --run`
    let Ok(flatpak_builder) = keyfile.boolean("Instance", "build") else {
        return Some(false);
    };

    let Ok(name) = keyfile.string("Application", "name") else {
        return Some(false);
    };

    Some(flatpak_builder && name.ends_with("Devel"))
}

/// Single thread that can be used for running one blocking tasks at a time
///
/// TODO: For builtin loaders, the threads are kept alive. For external, we don't retain them.
///
/// Reason: For builtin loaders, there is a risk that the memory usage would spread over too many arenas if we don't re-use the threads.
#[derive(Debug)]
pub enum DedicatedMainContext {
    Uninitialized,
    Initialized {
        main_context: glib::MainContext,
        main_loop: glib::MainLoop,
    },
}

impl Drop for DedicatedMainContext {
    fn drop(&mut self) {
        if let Self::Initialized {
            main_context,
            main_loop,
        } = self
        {
            // Run on the MainContext to ensure that we only quit after the loop is actually running
            main_context.invoke(glib::clone!(
                #[strong]
                main_loop,
                move || main_loop.quit()
            ));
        }
    }
}

impl DedicatedMainContext {
    pub fn new() -> Self {
        Self::Uninitialized
    }

    fn ensure_init(&mut self) -> glib::MainContext {
        match self {
            DedicatedMainContext::Initialized {
                main_context,
                main_loop: _,
            } => main_context.clone(),
            DedicatedMainContext::Uninitialized => {
                // TODO: Limit number of threads

                let main_context = glib::MainContext::new();
                let main_loop = glib::MainLoop::new(Some(&main_context), true);

                std::thread::spawn(glib::clone!(
                    #[strong]
                    main_context,
                    #[strong]
                    main_loop,
                    move || {
                        let res = main_context.with_thread_default(|| main_loop.run());
                        if let Err(err) = res {
                            tracing::error!(
                                "Failed to spawn main loop for dedicated thread: {err}"
                            );
                        }
                        tracing::debug!("Exiting deditacted thread.")
                    }
                ));

                *self = Self::Initialized {
                    main_context: main_context.clone(),
                    main_loop,
                };

                main_context
            }
        }
    }

    pub async fn unblock<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(
        &mut self,
        f: F,
    ) -> Result<T, ErrorKind> {
        self.ensure_init()
            .spawn(async { f() })
            .await
            .map_err(|x| ErrorKind::panic(x.into_panic()))
    }

    pub async fn unblock_once<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(
        f: F,
    ) -> Result<T, ErrorKind> {
        let mut self_ = Self::new();
        self_.unblock(f).await
    }
}

#[cfg(feature = "async-io")]
pub use async_io_utils::*;
#[cfg(feature = "tokio")]
pub use tokio_utils::*;

#[cfg(feature = "async-io")]
mod async_io_utils {
    use super::*;

    pub type AsyncMutex<T> = async_lock::Mutex<T>;

    pub const fn new_async_mutex<T>(t: T) -> AsyncMutex<T> {
        AsyncMutex::new(t)
    }

    #[cfg(feature = "external")]
    pub async fn read_dir<P: AsRef<Path>>(
        path: P,
    ) -> Result<
        impl Stream<Item = Result<PathBuf, Box<dyn std::error::Error + Sync + Send>>>,
        Box<dyn std::error::Error + Sync + Send>,
    > {
        Ok(async_fs::read_dir(path)
            .await?
            .map(|result| result.map(|entry| entry.path()).map_err(Into::into)))
    }

    pub use async_fs::read;

    pub async fn timeout_future(duration: std::time::Duration) {
        async_io::Timer::after(duration).await;
    }
}

#[cfg(feature = "tokio")]
mod tokio_utils {
    use super::*;

    pub use tokio::fs::read;

    pub type AsyncMutex<T> = tokio::sync::Mutex<T>;

    pub const fn new_async_mutex<T>(t: T) -> AsyncMutex<T> {
        AsyncMutex::const_new(t)
    }

    #[cfg(feature = "external")]
    pub async fn read_dir<P: AsRef<Path>>(
        path: P,
    ) -> Result<
        impl Stream<Item = Result<PathBuf, Box<dyn std::error::Error + Sync + Send>>>,
        Box<dyn std::error::Error + Sync + Send>,
    > {
        let read_dir = tokio::fs::read_dir(path).await?;

        Ok(tokio_stream::wrappers::ReadDirStream::new(read_dir)
            .map(|result| result.map(|entry| entry.path()).map_err(Into::into)))
    }

    pub async fn timeout_future(duration: std::time::Duration) {
        tokio::time::sleep(duration).await;
    }
}
