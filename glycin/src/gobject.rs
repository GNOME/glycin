pub mod creator;
pub mod encoded_image;
pub mod frame;
pub mod frame_request;
pub mod image;
pub mod loader;
pub mod new_frame;

use std::sync::OnceLock;

pub use creator::GlyCreator;
pub use encoded_image::GlyEncodedImage;
pub use frame::{GlyCicp, GlyFrame};
pub use frame_request::GlyFrameRequest;
pub use image::GlyImage;
pub use loader::GlyLoader;
pub use new_frame::GlyNewFrame;
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

        async_global_executor::init_with_config(
            async_global_executor::GlobalExecutorConfig::default()
                .with_min_threads(1)
                .with_thread_name_fn(|| String::from("gly-global-exec")),
        );
    });
}
