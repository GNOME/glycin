// Copyright (c) 2024 GNOME Foundation Inc.

//! Internal DBus API

use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures_channel::oneshot;
use futures_util::FutureExt;
use gio::glib;
use gio::prelude::*;
use glycin_common::Operations;
use glycin_utils::{
    CompleteEditorOutput, EditRequest, EncodedImage, EncodingOptions, FrameRequest, InitRequest,
    InitializationDetails, NewImage, RemoteEditableImage, RemoteError, RemoteImage, SharedMemory,
    SparseEditorOutput,
};
use nix::sys::signal;
use zbus::zvariant::{self, OwnedObjectPath};

use crate::sandbox::Sandbox;
use crate::util::{self, Task, spawn};
use crate::{
    DBusProxy, EditableImage, Error, ErrorKind, Image, MimeType, SandboxMechanism, config,
};

#[derive(Debug)]
pub struct RemoteProcess<P: DBusProxy> {
    dbus_connection: zbus::Connection,
    _dbus_connection_task: Task<()>,
    proxy: P,
    pub stderr_content: Arc<Mutex<String>>,
    pub stdout_content: Arc<Mutex<String>>,
    pub process_disconnected: Arc<AtomicBool>,
    cancellable: gio::Cancellable,
    base_dir: Option<PathBuf>,
}

impl<P: DBusProxy> Drop for RemoteProcess<P> {
    fn drop(&mut self) {
        tracing::debug!("Winding down process");
        self.cancellable.cancel();
    }
}

static_assertions::assert_impl_all!(RemoteProcess<LoaderProxy>: Send, Sync);
static_assertions::assert_impl_all!(RemoteProcess<EditorProxy>: Send, Sync);

pub trait ZbusProxy<'a>: Sized + Sync + Send + From<zbus::Proxy<'a>> {
    const TYPE: &'static str;
    fn builder(conn: &zbus::Connection) -> zbus::proxy::Builder<'a, Self>;
}

impl<'a> ZbusProxy<'a> for LoaderProxy<'a> {
    const TYPE: &'static str = "loader";
    fn builder(conn: &zbus::Connection) -> zbus::proxy::Builder<'a, Self> {
        Self::builder(conn)
    }
}

impl<'a> ZbusProxy<'a> for EditorProxy<'a> {
    const TYPE: &'static str = "editor";
    fn builder(conn: &zbus::Connection) -> zbus::proxy::Builder<'a, Self> {
        Self::builder(conn)
    }
}

impl<P: DBusProxy> RemoteProcess<P> {
    pub async fn new(
        config_entry: config::ConfigEntry,
        sandbox_mechanism: SandboxMechanism,
        base_dir: Option<PathBuf>,
        cancellable: &gio::Cancellable,
    ) -> Result<Self, Error> {
        // UnixStream which facilitates the D-Bus connection. The stream is passed as
        // stdin to loader binaries.
        let (unix_stream, loader_stdin) = std::os::unix::net::UnixStream::pair()?;
        unix_stream.set_nonblocking(true)?;
        loader_stdin.set_nonblocking(true)?;

        let mut sandbox = Sandbox::new(sandbox_mechanism, config_entry.clone(), loader_stdin)?;
        // Mount dir that contains the file as read only for formats like SVG
        if let Some(base_dir) = &base_dir {
            sandbox.add_ro_bind(base_dir.clone());
        }

        let spawned_sandbox = sandbox.spawn().await?;

        let command_dbg = format!("{:?}", spawned_sandbox.command);

        let (sender_child, child_process) = oneshot::channel();
        let (sender_child_return, child_return) = oneshot::channel();

        let process_disconnected = Arc::new(AtomicBool::new(false));

        // Spawning an extra thread to run and wait for the loader process since
        // PR_SET_PDEATHSIG in child processes is bound to the thread.
        std::thread::Builder::new()
            .name(format!("gly-hdl-{}", P::TYPE,))
            .spawn(glib::clone!(
                #[strong]
                process_disconnected,
                move || {
                    let mut command = spawned_sandbox.command;
                    let command_dbg = format!("{:?}", command);

                    tracing::debug!("Spawning loader/editor:\n    {command_dbg}");
                    let mut child = match command.spawn() {
                        Ok(mut child) => {
                            let id = child.id();
                            let info = Ok((child.stderr.take(), child.stdout.take(), id));
                            if let Err(err) = sender_child.send(info) {
                                tracing::info!(
                                "Failed to inform coordinating thread about process state: {err:?}"
                            );
                            }
                            child
                        }
                        Err(err) => {
                            let err = if err.kind() == std::io::ErrorKind::NotFound {
                                ErrorKind::SpawnErrorNotFound {
                                    cmd: command_dbg.clone(),
                                    err: Arc::new(err),
                                }
                            } else {
                                ErrorKind::SpawnError {
                                    cmd: command_dbg.clone(),
                                    err: Arc::new(err),
                                }
                            };
                            tracing::debug!("Failed to spawn process: {err}");
                            if let Err(err) = sender_child.send(Err(err)) {
                                tracing::info!(
                                "Failed to inform coordinating thread about process state: {err:?}"
                            );
                            }
                            return;
                        }
                    };

                    let result = child.wait();
                    process_disconnected.store(true, Ordering::Relaxed);
                    tracing::debug!(
                        "Process exited: {:?} {result:?}",
                        result.as_ref().ok().map(|x| x.code())
                    );
                    if let Err(err) = sender_child_return.send(result) {
                        tracing::debug!(
                            "Failed to send process return value to coordinating thread: {err:?}"
                        );
                    }
                }
            ))?;

        let mut child_process = child_process.await??;

        let stderr_content: Arc<Mutex<String>> = Default::default();
        spawn_stdio_reader(
            &mut child_process.0,
            &stderr_content,
            process_disconnected.clone(),
            "stderr",
        );

        let stdout_content: Arc<Mutex<String>> = Default::default();
        spawn_stdio_reader(
            &mut child_process.1,
            &stdout_content,
            process_disconnected.clone(),
            "stdout",
        );

        #[cfg(feature = "tokio")]
        let unix_stream = tokio::net::UnixStream::from_std(unix_stream)?;

        let guid = zbus::Guid::generate();
        let dbus_result = zbus::connection::Builder::unix_stream(unix_stream)
            .p2p()
            .server(guid)?
            .auth_mechanism(zbus::AuthMechanism::Anonymous)
            .internal_executor(false)
            .build()
            .shared();

        let subprocess_id = nix::unistd::Pid::from_raw(child_process.2.try_into().unwrap());

        futures_util::select! {
            _result = dbus_result.clone().fuse() => Ok(()),
            _result = cancellable.future().fuse() => {
                tracing::debug!("Killing process due to cancellation.");
                let _result = signal::kill(subprocess_id, signal::Signal::SIGKILL);
                Err(glib::Error::from(gio::Cancelled).into())
            },
            return_status = child_return.fuse() => {
                match return_status? {
                    Ok(status) => Err(ErrorKind::PrematureExit { status, cmd: command_dbg.clone() }.err()),
                    Err(err) => Err(ErrorKind::StdIoError{ err: Arc::new(err), info: command_dbg.clone() }.err()),
                }
            }
        }?;

        cancellable.connect_cancelled(move |_| {
            tracing::debug!("Killing process due to cancellation (late): {command_dbg}");
            let _result = signal::kill(subprocess_id, signal::Signal::SIGKILL);
        });

        let dbus_connection = dbus_result.await?;

        let dbus_connection_task = spawn(glib::clone!(
            #[strong]
            dbus_connection,
            async move {
                let executor = dbus_connection.executor();
                loop {
                    executor.tick().await;
                }
            }
        ));

        let decoding_instruction = P::builder(&dbus_connection)
            // Unused since P2P connection
            .destination("org.gnome.glycin")?
            .path("/org/gnome/glycin")?
            .build()
            .await?;

        Ok(Self {
            dbus_connection,
            _dbus_connection_task: dbus_connection_task,
            proxy: decoding_instruction,
            stderr_content,
            stdout_content,
            process_disconnected,
            cancellable: cancellable.clone(),
            base_dir,
        })
    }

    fn init_request(
        &self,
        mime_type: &MimeType,
        external_reader: OwnedFd,
    ) -> Result<InitRequest, Error> {
        let fd = zvariant::OwnedFd::from(external_reader);

        let mime_type = mime_type.to_string();

        let mut details = InitializationDetails::default();
        details.base_dir = self.base_dir.clone();

        Ok(InitRequest {
            fd,
            mime_type,
            details,
        })
    }
}

impl RemoteProcess<LoaderProxy<'static>> {
    pub async fn init(
        &self,
        mime_type: &MimeType,
        external_reader: OwnedFd,
    ) -> Result<RemoteImage<SharedMemory>, Error> {
        let init_request = self.init_request(mime_type, external_reader)?;

        let image_info = self.proxy.init(init_request).await?;

        Ok(image_info)
    }

    pub async fn done(self: Arc<Self>, frame_request_path: OwnedObjectPath) -> Result<(), Error> {
        let loader_proxy = LoaderStateProxy::builder(&self.dbus_connection)
            .destination("org.gnome.glycin")?
            .path(frame_request_path)?
            .build()
            .await?;

        loader_proxy.done().await.map_err(Into::into)
    }

    pub async fn request_frame(
        &self,
        frame_request: FrameRequest,
        image: &Image,
    ) -> Result<glycin_utils::Frame<SharedMemory>, Error> {
        let frame_request_path = image.frame_request_path();

        let loader_proxy = LoaderStateProxy::builder(&self.dbus_connection)
            .destination("org.gnome.glycin")?
            .path(frame_request_path)?
            .build()
            .await?;

        loader_proxy.frame(frame_request).await.map_err(Into::into)
    }
}

impl RemoteProcess<EditorProxy<'static>> {
    pub async fn create(
        &self,
        mime_type: &MimeType,
        new_image: NewImage<SharedMemory>,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage<SharedMemory>, Error> {
        self.proxy
            .create(mime_type.to_string(), new_image, encoding_options)
            .await
            .map_err(Into::into)
    }

    pub async fn edit(
        &self,
        external_reader: OwnedFd,
        mime_type: &MimeType,
    ) -> Result<RemoteEditableImage, Error> {
        let init_request = self.init_request(mime_type, external_reader)?;

        self.proxy.edit(init_request).await.map_err(Into::into)
    }

    pub async fn editor_apply_sparse(
        &self,
        operations: &Operations,
        editable_image: &EditableImage,
    ) -> Result<SparseEditorOutput<SharedMemory>, Error> {
        let editor_proxy = EditableImageProxy::builder(&self.dbus_connection)
            .destination("org.gnome.glycin")?
            .path(editable_image.edit_request_path())?
            .build()
            .await?;

        let edit_request = EditRequest::for_operations(operations)?;

        editor_proxy
            .apply_sparse(edit_request)
            .await
            .map_err(Into::into)
    }

    pub async fn editor_apply_complete(
        &self,
        operations: &Operations,
        editable_image: &EditableImage,
    ) -> Result<CompleteEditorOutput<SharedMemory>, Error> {
        let editor_proxy = EditableImageProxy::builder(&self.dbus_connection)
            .destination("org.gnome.glycin")?
            .path(editable_image.edit_request_path())?
            .build()
            .await?;

        let edit_request = EditRequest::for_operations(operations)?;

        editor_proxy
            .apply_complete(edit_request)
            .await
            .map_err(Into::into)
    }

    pub fn done_background(self: Arc<Self>, image: &EditableImage) {
        let edit_request_path = image.edit_request_path();
        let arc = self.clone();

        util::spawn_detached(arc.done(edit_request_path));
    }

    pub async fn done(self: Arc<Self>, edit_request_path: OwnedObjectPath) -> Result<(), Error> {
        let loader_proxy = EditableImageProxy::builder(&self.dbus_connection)
            .destination("org.gnome.glycin")?
            .path(edit_request_path)?
            .build()
            .await?;

        loader_proxy.done().await.map_err(Into::into)
    }
}

#[zbus::proxy(interface = "org.gnome.glycin.Loader")]
pub trait Loader {
    async fn init(
        &self,
        init_request: InitRequest,
    ) -> Result<RemoteImage<SharedMemory>, RemoteError>;
}

#[zbus::proxy(name = "org.gnome.glycin.Image")]
pub trait LoaderState {
    async fn frame(
        &self,
        frame_request: FrameRequest,
    ) -> Result<glycin_utils::Frame<SharedMemory>, RemoteError>;
    async fn done(&self) -> Result<(), RemoteError>;
}

#[zbus::proxy(
    interface = "org.gnome.glycin.Editor",
    default_path = "/org/gnome/glycin"
)]
pub trait Editor {
    async fn create(
        &self,
        mime_type: String,
        new_image: NewImage<SharedMemory>,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage<SharedMemory>, RemoteError>;

    async fn edit(&self, init_request: InitRequest) -> Result<RemoteEditableImage, RemoteError>;
}

#[zbus::proxy(interface = "org.gnome.glycin.EditableImage")]
pub trait EditableImage {
    async fn apply_sparse(
        &self,
        edit_request: EditRequest,
    ) -> Result<SparseEditorOutput<SharedMemory>, RemoteError>;

    async fn apply_complete(
        &self,
        edit_request: EditRequest,
    ) -> Result<CompleteEditorOutput<SharedMemory>, RemoteError>;

    async fn done(&self) -> Result<(), RemoteError>;
}

#[cfg(not(feature = "tokio"))]
fn spawn_stdio_reader(
    stdio: &mut Option<impl Read + Send + std::os::fd::AsFd + async_io::IoSafe + 'static>,
    store: &Arc<Mutex<String>>,
    process_disconnected: Arc<AtomicBool>,
    name: &'static str,
) {
    use futures_lite::AsyncBufReadExt;
    if let Some(stdio) = stdio.take() {
        let store = store.clone();
        util::spawn_detached(async move {
            match async_io::Async::new(stdio) {
                Err(err) => {
                    tracing::error!("Can't read {name}: {err}");
                }
                Ok(read_stdio) => {
                    let mut read_stdio = futures_lite::io::BufReader::new(read_stdio);

                    let mut buf = String::new();
                    loop {
                        match read_stdio.read_line(&mut buf).await {
                            Ok(len) => {
                                if len == 0 {
                                    process_disconnected.store(true, Ordering::Relaxed);
                                    tracing::debug!("{name} disconnected without error");
                                    break;
                                }
                                tracing::debug!("Loader {name}: {buf}", buf = buf.trim_end());
                                store.lock().unwrap().push_str(&buf);
                                buf.clear();
                            }
                            Err(err) => {
                                process_disconnected.store(true, Ordering::Relaxed);
                                tracing::debug!("{name} disconnected with error: {err}");
                                break;
                            }
                        }
                    }
                }
            }
        });
    }
}

#[cfg(feature = "tokio")]
fn spawn_stdio_reader(
    stdio: &mut Option<impl Read + Send + 'static>,
    store: &Arc<Mutex<String>>,
    process_disconnected: Arc<AtomicBool>,
    name: &'static str,
) {
    use std::io::{BufRead, BufReader};
    if let Some(stdout) = stdio.take() {
        let store = store.clone();
        let _ = std::thread::Builder::new()
            .name(format!("gly-{name}"))
            .spawn(move || {
                let mut stdout = BufReader::new(stdout);

                let mut buf = String::new();
                loop {
                    match stdout.read_line(&mut buf) {
                        Ok(len) => {
                            if len == 0 {
                                process_disconnected.store(true, Ordering::Relaxed);
                                tracing::debug!("{name} disconnected without error");
                                break;
                            }
                            tracing::debug!("Loader {name}: {buf}", buf = buf.trim_end());
                            store.lock().unwrap().push_str(&buf);
                            buf.clear();
                        }
                        Err(err) => {
                            process_disconnected.store(true, Ordering::Relaxed);
                            tracing::debug!("{name} disconnected with error: {err}");
                            break;
                        }
                    }
                }
            });
    }
}
