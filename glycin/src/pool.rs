static DEFAULT_POOL: LazyLock<Pool> = LazyLock::new(Pool::new);

use crate::{config, dbus, Error, MimeType, SandboxMechanism};
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

type Loader<'a> = dbus::RemoteProcess<'a, dbus::LoaderProxy<'a>>;

#[derive(Clone)]
pub struct PooledLoader<'a> {
    last_use: Instant,
    loader: Arc<Loader<'a>>,
}
pub struct Pool<'a> {
    loaders: Mutex<BTreeMap<config::ImageLoaderConfig, PooledLoader<'a>>>,
    loader_retention_time: Duration,
}

impl<'a> Pool<'a> {
    pub fn new() -> Self {
        Self {
            loader_retention_time: Duration::from_secs(60),
            loaders: Default::default(),
        }
    }

    pub fn global() -> &'static Self {
        &*DEFAULT_POOL
    }

    pub async fn get_loader(
        &self,
        loader_config: config::ImageLoaderConfig,
        mime_type: &config::MimeType,
        sandbox_mechanism: SandboxMechanism,
        file: Option<gio::File>,
        cancellable: &gio::Cancellable,
    ) -> Result<Arc<Loader<'a>>, Error> {
        let pooled_loader = self.loaders.lock().unwrap().get(&loader_config).cloned();

        if let Some(loader) = pooled_loader {
            dbg!("existing loader");
            Ok(loader.loader)
        } else {
            dbg!("NEW LOADER");

            let process = Arc::new(
                dbus::RemoteProcess::new(
                    &mime_type,
                    loader_config.clone(),
                    sandbox_mechanism,
                    file.clone(),
                    cancellable,
                )
                .await?,
            );

            self.loaders.lock().unwrap().insert(
                loader_config,
                PooledLoader {
                    last_use: Instant::now(),
                    loader: process.clone(),
                },
            );

            Ok(process)
        }
    }

    pub fn clean_loaders(&self) {}
}
