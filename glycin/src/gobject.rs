pub mod frame;
pub mod frame_request;
pub mod image;
pub mod loader;

use std::sync::OnceLock;

pub use frame::GlyFrame;
pub use frame_request::GlyFrameRequest;
pub use image::GlyImage;
pub use loader::GlyLoader;
use tracing_subscriber::layer::*;
use tracing_subscriber::util::*;

pub fn init() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let default_level =
            if !glib::log_writer_default_would_drop(glib::LogLevel::Debug, Some("glycin")) {
                tracing_subscriber::filter::LevelFilter::DEBUG
            } else {
                tracing_subscriber::filter::LevelFilter::ERROR
            };

        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::builder()
                    .with_default_directive(default_level.into())
                    .from_env_lossy(),
            )
            .with(tracing_subscriber::fmt::Layer::default().compact())
            .init();

        tracing::debug!("Initialized logging");
    });
}
