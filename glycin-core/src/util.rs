use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use gio::glib;
use gio::prelude::CancellableExtManual;
#[cfg(feature = "gdk4")]
use glycin_utils::MemoryFormat;

#[cfg(feature = "gdk4")]
use crate::ColorState;
#[cfg(feature = "external")]
use crate::sandbox::Sandbox;

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
            futures_util::future::Either::Left(_) => Err(crate::ErrorKind::Canceled(None).err()),
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
            futures_util::future::Either::Left(_) => Err(crate::ErrorKind::Timeout(timeout).err()),
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
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RunEnvironment {
    /// Not inside Flatpak
    Host,

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
            let run_env = if let Some(devel) = flatpak_devel().await {
                if devel {
                    Self::FlatpakDevel
                } else {
                    Self::Flatpak
                }
            } else if Sandbox::check_bwrap_syscalls_blocked().await {
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

pub async fn spawn_blocking<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(f: F) -> T {
    // TODO: Check if more panic handlings do make sense here
    gio::spawn_blocking(f).await.unwrap()
}

#[cfg(feature = "async-io")]
pub use async_io_utils::*;
#[cfg(feature = "tokio")]
pub use tokio_utils::*;

#[cfg(feature = "async-io")]
mod async_io_utils {
    use super::*;

    #[cfg(feature = "external")]
    pub type Task<T> = async_task::Task<T>;

    #[cfg(feature = "external")]
    pub fn spawn<T: Send + 'static>(
        f: impl std::future::Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        async_global_executor::spawn(f)
    }

    #[cfg(feature = "external")]
    pub fn spawn_detached<F>(f: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        async_global_executor::spawn(f).detach()
    }

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

    #[cfg(feature = "external")]
    pub type TimerHandle = async_global_executor::Task<()>;

    #[cfg(feature = "external")]
    pub fn spawn_timeout(
        duration: std::time::Duration,
        f: impl Future + Send + 'static,
    ) -> TimerHandle {
        async_global_executor::spawn(async move {
            async_io::Timer::after(duration).await;
            f.await;
        })
    }

    pub async fn timeout_future(duration: std::time::Duration) {
        async_io::Timer::after(duration).await;
    }
}

#[cfg(feature = "tokio")]
mod tokio_utils {
    use super::*;

    #[cfg(feature = "external")]
    pub type Task<T> = tokio::task::JoinHandle<T>;

    pub use tokio::fs::read;

    #[cfg(feature = "external")]
    pub fn spawn<T: Send + 'static>(
        f: impl std::future::Future<Output = T> + Send + 'static,
    ) -> Task<T> {
        tokio::task::spawn(f)
    }

    #[cfg(feature = "external")]
    pub fn spawn_detached<F>(f: F)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        tokio::task::spawn(f);
    }

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

    #[cfg(feature = "external")]
    #[derive(Debug)]
    pub struct TimerHandle(tokio::task::JoinHandle<()>);

    #[cfg(feature = "external")]
    impl Drop for TimerHandle {
        fn drop(&mut self) {
            self.0.abort();
        }
    }

    #[cfg(feature = "external")]
    pub fn spawn_timeout(
        duration: std::time::Duration,
        f: impl Future + Send + 'static,
    ) -> TimerHandle {
        TimerHandle(tokio::task::spawn(async move {
            tokio::time::sleep(duration).await;
            f.await;
        }))
    }

    pub async fn timeout_future(duration: std::time::Duration) {
        tokio::time::sleep(duration).await;
    }
}
