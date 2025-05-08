static DEFAULT_POOL: LazyLock<Arc<Pool>> = LazyLock::new(Pool::new);

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use gio::glib;
use gio::prelude::{CancellableExt, CancellableExtManual};

use crate::config::{ConfigEntry, ConfigEntryHash};
use crate::dbus::ZbusProxy;
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
    loaders:
        Mutex<BTreeMap<config::ConfigEntryHash, Arc<PooledProcess<dbus::LoaderProxy<'static>>>>>,
    editors:
        Mutex<BTreeMap<config::ConfigEntryHash, Arc<PooledProcess<dbus::EditorProxy<'static>>>>>,
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
        file: Option<gio::File>,
        cancellable: &gio::Cancellable,
        loader_alive: std::sync::Weak<()>,
    ) -> Result<Arc<PooledProcess<dbus::LoaderProxy<'static>>>, Error> {
        let pooled_loaders = self.loaders.lock().unwrap();

        let pp = self
            .get_process(
                pooled_loaders,
                ConfigEntry::Loader(loader_config.clone()),
                sandbox_mechanism,
                file,
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
        file: Option<gio::File>,
        cancellable: &gio::Cancellable,
        editor_alive: std::sync::Weak<()>,
    ) -> Result<Arc<PooledProcess<dbus::EditorProxy<'static>>>, Error> {
        let pooled_editors = self.editors.lock().unwrap();

        let pp = self
            .get_process(
                pooled_editors,
                ConfigEntry::Editor(editor_config.clone()),
                sandbox_mechanism,
                file,
                cancellable,
                editor_alive,
            )
            .await?;

        Ok(pp)
    }

    pub async fn get_process<P: ZbusProxy<'static> + 'static>(
        &self,
        mut pooled_processes: MutexGuard<'_, BTreeMap<ConfigEntryHash, Arc<PooledProcess<P>>>>,
        config: config::ConfigEntry,
        sandbox_mechanism: SandboxMechanism,
        file: Option<gio::File>,
        cancellable: &gio::Cancellable,
        alive: std::sync::Weak<()>,
    ) -> Result<Arc<PooledProcess<P>>, Error> {
        let pooled_process = pooled_processes.get(&config.hash_value()).cloned();

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

        tracing::debug!("Creating new loader.");

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
                file.clone(),
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

        pooled_processes.insert(config.hash_value(), pp.clone());

        Ok(pp)
    }

    pub fn clean_loaders(&self) {
        let mut loaders = self.loaders.lock().unwrap();

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
