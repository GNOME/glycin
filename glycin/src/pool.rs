static DEFAULT_POOL: LazyLock<Arc<Pool>> = LazyLock::new(Pool::new);

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use gio::glib;
use gio::prelude::{CancellableExt, CancellableExtManual};

use crate::dbus::ZbusProxy;
use crate::{config, dbus, Error, SandboxMechanism};

type Loader = dbus::RemoteProcess<dbus::LoaderProxy<'static>>;

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
        Mutex<BTreeMap<config::ImageLoaderConfig, Arc<PooledProcess<dbus::LoaderProxy<'static>>>>>,
    loader_retention_time: Duration,
}

impl Pool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            loader_retention_time: Duration::from_secs(60),
            loaders: Default::default(),
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
        let pooled_loader = self.loaders.lock().unwrap().get(&loader_config).cloned();

        if let Some(loader) = pooled_loader {
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
                loader_config.clone(),
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
            users: vec![loader_alive],
        });

        self.loaders
            .lock()
            .unwrap()
            .insert(loader_config, pp.clone());

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
                    cfg.exec,
                    Arc::strong_count(&loader.process)
                )
            }
            !drop
        });
    }
}
