// Copyright (c) 2024 GNOME Foundation Inc.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::raw::{c_int, c_void};
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

use nix::libc::{c_uint, siginfo_t};

use crate::dbus::*;
use crate::error::*;

pub struct Communication {
    _dbus_connection: zbus::Connection,
}

impl Communication {
    pub fn spawn(decoder: impl LoaderImplementation + 'static) {
        Self::setup_sigsys_handler();

        futures_lite::future::block_on(async move {
            let _connection = Communication::new(decoder).await;
            std::future::pending::<()>().await;
        })
    }

    pub async fn new(decoder: impl LoaderImplementation + 'static) -> Self {
        let unix_stream = unsafe { UnixStream::from_raw_fd(std::io::stdin().as_raw_fd()) };

        let instruction_handler = Loader {
            decoder: Mutex::new(Box::new(decoder)),
        };
        let dbus_connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
            .p2p()
            .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
            .serve_at("/org/gnome/glycin", instruction_handler)
            .expect("Failed to setup instruction handler")
            .build()
            .await
            .expect("Failed to create private DBus connection");

        Communication {
            _dbus_connection: dbus_connection,
        }
    }

    fn setup_sigsys_handler() {
        let mut mask = nix::sys::signal::SigSet::empty();
        mask.add(nix::sys::signal::Signal::SIGSYS);

        let sigaction = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::SigAction(Self::sigsys_handler),
            nix::sys::signal::SaFlags::SA_SIGINFO,
            mask,
        );

        unsafe {
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGSYS, &sigaction).unwrap()
        };
    }

    #[allow(non_camel_case_types)]
    extern "C" fn sigsys_handler(_: c_int, info: *mut siginfo_t, _: *mut c_void) {
        // Reimplement siginfo_t since the libc crate doesn't support _sigsys
        // information
        #[repr(C)]
        struct siginfo_t {
            si_signo: c_int,
            si_errno: c_int,
            si_code: c_int,
            _sifields: _sigsys,
        }

        #[repr(C)]
        struct _sigsys {
            _call_addr: *const c_void,
            _syscall: c_int,
            _arch: c_uint,
        }

        let info: *mut siginfo_t = info.cast();
        let syscall = unsafe { info.as_ref().unwrap()._sifields._syscall };

        let name = libseccomp::ScmpSyscall::from(syscall).get_name().ok();

        eprintln!(
            "glycin sandbox: Blocked syscall used: '{}' ({})",
            name.unwrap_or_else(|| String::from("Unknown Syscall")),
            syscall
        );
    }
}

pub trait LoaderImplementation: Send {
    fn init(
        &self,
        stream: UnixStream,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<ImageInfo, LoaderError>;
    fn frame(&self, frame_request: FrameRequest) -> Result<Frame, LoaderError>;
}

pub struct Loader {
    pub decoder: Mutex<Box<dyn LoaderImplementation>>,
}

#[zbus::interface(name = "org.gnome.glycin.Loader")]
impl Loader {
    async fn init(&self, init_request: InitRequest) -> Result<ImageInfo, RemoteError> {
        let fd = OwnedFd::from(init_request.fd);
        let stream = UnixStream::from(fd);

        let image_info = self
            .decoder
            .lock()
            .or_else(|err| {
                Err(RemoteError::InternalLoaderError(format!(
                    "Failed to lock decoder for init(): {err}"
                )))
            })?
            .init(stream, init_request.mime_type, init_request.details)?;

        Ok(image_info)
    }

    async fn frame(&self, frame_request: FrameRequest) -> Result<Frame, RemoteError> {
        self.decoder
            .lock()
            .or_else(|err| {
                Err(RemoteError::InternalLoaderError(format!(
                    "Failed to lock decoder for frame(): {err}"
                )))
            })?
            .frame(frame_request)
            .map_err(Into::into)
    }
}
