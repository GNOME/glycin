static DEFAULT_POOL: LazyLock<Arc<Pool>> = LazyLock::new(|| Arc::new(Pool::default()));

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use std::usize;

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

    pub fn n_users(&self) -> usize {
        self.users
            .iter()
            .map(|x| (x.strong_count() > 0) as usize)
            .sum()
    }
}

#[derive(Debug, Default)]
pub struct Pool {
    loaders: AsyncMutex<
        BTreeMap<config::ConfigEntryHash, Vec<Arc<PooledProcess<dbus::LoaderProxy<'static>>>>>,
    >,
    editors: AsyncMutex<
        BTreeMap<config::ConfigEntryHash, Vec<Arc<PooledProcess<dbus::EditorProxy<'static>>>>>,
    >,
    config: PoolConfig,
}

#[derive(Debug)]
pub struct PoolConfig {
    loader_retention_time: Duration,
    max_parallel_operations: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            loader_retention_time: Duration::from_secs(60),
            max_parallel_operations: usize::MAX,
        }
    }
}

impl PoolConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_parallel_operations(&mut self, max_parallel_operations: usize) -> &mut Self {
        if max_parallel_operations == 0 {
            self.max_parallel_operations = usize::MAX;
        } else {
            self.max_parallel_operations = max_parallel_operations;
        }
        self
    }
}

impl Pool {
    pub fn new(config: PoolConfig) -> Arc<Self> {
        let mut pool = Self::default();
        pool.config = config;

        Arc::new(pool)
    }

    pub fn global() -> Arc<Self> {
        DEFAULT_POOL.clone()
    }

    pub(crate) async fn get_loader(
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

    pub(crate) async fn get_editor(
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

    pub(crate) async fn get_process<P: ZbusProxy<'static> + 'static>(
        &self,
        pooled_processes: &AsyncMutex<BTreeMap<ConfigEntryHash, Vec<Arc<PooledProcess<P>>>>>,
        config: config::ConfigEntry,
        sandbox_mechanism: SandboxMechanism,
        base_dir: Option<PathBuf>,
        cancellable: &gio::Cancellable,
        alive: std::sync::Weak<()>,
    ) -> Result<Arc<PooledProcess<P>>, Error> {
        let config_hash = config.hash_value(base_dir.clone(), sandbox_mechanism);
        let mut pooled_processes = pooled_processes.lock().await;
        let pooled_processes = pooled_processes.entry(config_hash).or_default();

        for process in pooled_processes.iter() {
            if process.process.process_disconnected.load(Ordering::Relaxed) {
                tracing::debug!("Existing loader/editor in pool is disconnected. Trying next.");
            } else if process.n_users() >= self.config.max_parallel_operations {
                tracing::debug!(
                    "Existing loader/editor in pool is at 'max_parallel_operations'. Trying next."
                );
            } else {
                tracing::debug!("Using existing loader from pool.");
                return Ok(process.clone());
            }
        }

        tracing::debug!("No existing loader/editor in pool. Spawning new one.");

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

        pooled_processes.push(pp.clone());

        Ok(pp)
    }

    pub(crate) async fn clean_loaders(self: Arc<Self>) {
        let mut loader_map = self.loaders.lock().await;

        for (cfg, loaders) in loader_map.iter_mut() {
            loaders.retain(|loader| {
                let drop = loader.n_users() == 0
                    && loader.last_use.lock().unwrap().elapsed()
                        > self.config.loader_retention_time;
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
}
