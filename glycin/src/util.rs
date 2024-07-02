use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use futures_util::{Stream, StreamExt};
#[cfg(feature = "gdk4")]
use glycin_utils::MemoryFormat;

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

pub async fn is_flatpaked() -> bool {
    static IS_FLATPAKED: OnceLock<bool> = OnceLock::new();
    if let Some(result) = IS_FLATPAKED.get() {
        *result
    } else {
        let flatpaked = spawn_blocking(|| Path::new("/.flatpak-info").is_file()).await;
        *IS_FLATPAKED.get_or_init(|| flatpaked)
    }
}

#[cfg(not(feature = "tokio"))]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    async_io::block_on(future)
}

#[cfg(feature = "tokio")]
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    static TOKIO_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime =
        TOKIO_RT.get_or_init(|| tokio::runtime::Runtime::new().expect("tokio runtime was created"));
    runtime.block_on(future)
}

#[cfg(not(feature = "tokio"))]
pub async fn spawn_blocking<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(f: F) -> T {
    blocking::unblock(f).await
}

#[cfg(feature = "tokio")]
pub async fn spawn_blocking<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(f: F) -> T {
    tokio::task::spawn_blocking(f)
        .await
        .expect("task was not aborted")
}

#[cfg(not(feature = "tokio"))]
pub fn spawn_blocking_detached<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(f: F) {
    blocking::unblock(f).detach()
}

#[cfg(feature = "tokio")]
pub fn spawn_blocking_detached<F: FnOnce() -> T + Send + 'static, T: Send + 'static>(f: F) {
    tokio::task::spawn_blocking(f);
}

#[cfg(not(feature = "tokio"))]
pub type AsyncMutex<T> = async_lock::Mutex<T>;

#[cfg(not(feature = "tokio"))]
pub const fn new_async_mutex<T>(t: T) -> AsyncMutex<T> {
    AsyncMutex::new(t)
}

#[cfg(feature = "tokio")]
pub type AsyncMutex<T> = tokio::sync::Mutex<T>;

#[cfg(feature = "tokio")]
pub const fn new_async_mutex<T>(t: T) -> AsyncMutex<T> {
    AsyncMutex::const_new(t)
}

#[cfg(not(feature = "tokio"))]
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

#[cfg(feature = "tokio")]
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

#[cfg(not(feature = "tokio"))]
pub use async_fs::read;
#[cfg(feature = "tokio")]
pub use tokio::fs::read;

#[cfg(not(feature = "tokio"))]
pub async fn sleep(duration: std::time::Duration) {
    futures_timer::Delay::new(duration).await
}

#[cfg(feature = "tokio")]
pub use tokio::time::sleep;
