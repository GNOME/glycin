static DEFAULT_POOL: LazyLock<Arc<Pool>> = LazyLock::new(Pool::new);

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use gio::glib;
use gio::prelude::*;

use crate::config::{ConfigEntry, ConfigEntryHash};
use crate::dbus::ZbusProxy;
use crate::util::AsyncMutex;
use crate::{config, dbus, Error, SandboxMechanism};

#[derive(Debug)]
pub struct PooledProcess<P: ZbusProxy<'static> + 'static> {
    last_use: Mutex<Instant>,
    process: Arc<dbus::RemoteProcess<P>>,
    users: Vec<std::sync::Weak<()>>,
}

impl<P: ZbusProxy<'static> + 'static> PooledProcess<P> {
    pub fn use_(&self) -> Arc<dbus::RemoteProcess<P>> {
        *self.last_use.lock().unwrap() = Instant::now();
        self.process.clone()
    }
}

#[derive(Debug)]
pub struct Pool {
    loaders: AsyncMutex<
        BTreeMap<config::ConfigEntryHash, Arc<PooledProcess<dbus::LoaderProxy<'static>>>>,
    >,
    editors: AsyncMutex<
        BTreeMap<config::ConfigEntryHash, Arc<PooledProcess<dbus::EditorProxy<'static>>>>,
    >,
    loader_retention_time: Duration,
}

impl Pool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            loader_retention_time: Duration::from_secs(60),
            loaders: Default::default(),
            editors: Default::default(),
        })
    }

    pub fn global() -> Arc<Self> {
        DEFAULT_POOL.clone()
    }

    pub async fn get_loader(
        &self,
        loader_config: config::ImageLoaderConfig,
        sandbox_mechanism: SandboxMechanism,
        base_dir: Option<PathBuf>,
        cancellable: &gio::Cancellable,
        loader_alive: std::sync::Weak<()>,
    ) -> Result<Arc<PooledProcess<dbus::LoaderProxy<'static>>>, Error> {
        let pooled_loaders = &self.loaders;

        let pp = self
            .get_process(
                pooled_loaders,
                ConfigEntry::Loader(loader_config.clone()),
                sandbox_mechanism,
                base_dir,
                cancellable,
                loader_alive,
            )
            .await?;

        Ok(pp)
    }

    pub async fn get_editor(
        &self,
        editor_config: config::ImageEditorConfig,
        sandbox_mechanism: SandboxMechanism,
        base_dir: Option<PathBuf>,
        cancellable: &gio::Cancellable,
        editor_alive: std::sync::Weak<()>,
    ) -> Result<Arc<PooledProcess<dbus::EditorProxy<'static>>>, Error> {
        let pooled_editors = &self.editors;

        let pp = self
            .get_process(
                pooled_editors,
                ConfigEntry::Editor(editor_config.clone()),
                sandbox_mechanism,
                base_dir,
                cancellable,
                editor_alive,
            )
            .await?;

        Ok(pp)
    }

    pub async fn get_process<P: ZbusProxy<'static> + 'static>(
        &self,
        pooled_processes: &AsyncMutex<BTreeMap<ConfigEntryHash, Arc<PooledProcess<P>>>>,
        config: config::ConfigEntry,
        sandbox_mechanism: SandboxMechanism,
        base_dir: Option<PathBuf>,
        cancellable: &gio::Cancellable,
        alive: std::sync::Weak<()>,
    ) -> Result<Arc<PooledProcess<P>>, Error> {
        let mut pooled_processes = pooled_processes.lock().await;

        let config_hash = config.hash_value(base_dir.clone(), sandbox_mechanism);

        let pooled_process = pooled_processes.get(&config_hash).cloned();

        if let Some(loader) = pooled_process {
            if loader.process.process_disconnected.load(Ordering::Relaxed) {
                tracing::debug!(
                    "Existing loader in pool is disconnected. Dropping existing loader."
                );
            } else {
                tracing::debug!("Using existing loader from pool.");
                return Ok(loader);
            }
        }

        tracing::debug!("Now existing loader/editor in pool. Spawning new one.");

        let process_cancellable = gio::Cancellable::new();
        let Some(process_cancellable_tie) = cancellable.connect_cancelled(glib::clone!(
            #[weak]
            process_cancellable,
            move |_| process_cancellable.cancel()
        )) else {
            return Err(Error::Canceled(None));
        };

        let process = Arc::new(
            dbus::RemoteProcess::new(
                config.clone(),
                sandbox_mechanism,
                base_dir,
                &process_cancellable,
            )
            .await?,
        );

        cancellable.disconnect_cancelled(process_cancellable_tie);

        let pp = Arc::new(PooledProcess {
            last_use: Mutex::new(Instant::now()),
            process: process.clone(),
            users: vec![alive],
        });

        pooled_processes.insert(config_hash, pp.clone());

        Ok(pp)
    }

    pub async fn clean_loaders(self: Arc<Self>) {
        let mut loaders = self.loaders.lock().await;

        loaders.retain(|cfg, loader| {
            let drop = loader.users.iter().all(|x| x.strong_count() == 0)
                && loader.last_use.lock().unwrap().elapsed() > self.loader_retention_time;
            if drop {
                tracing::debug!(
                    "Dropping loader {:?} {}",
                    cfg.exec(),
                    Arc::strong_count(&loader.process)
                )
            }
            !drop
        });
    }
}
