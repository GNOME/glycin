// Copyright (c) 2024 GNOME Foundation Inc.

use std::ffi::{c_int, c_void};
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

use nix::libc::{c_uint, siginfo_t};

use crate::dbus_editor_api::{Editor, EditorImplementation, VoidEditorImplementation};
use crate::dbus_loader_api::{Loader, LoaderImplementation};

pub struct DbusServer {
    _dbus_connection: zbus::Connection,
}

impl DbusServer {
    pub fn spawn_loader<L: LoaderImplementation>(description: String) {
        futures_lite::future::block_on(async move {
            let _connection = Self::connect::<L, VoidEditorImplementation>(description).await;
            std::future::pending::<()>().await;
        })
    }

    pub fn spawn_loader_editor<L: LoaderImplementation, E: EditorImplementation>(
        description: String,
    ) {
        futures_lite::future::block_on(async move {
            let _connection = Self::connect::<L, E>(description).await;
            std::future::pending::<()>().await;
        })
    }

    async fn connect<L: LoaderImplementation, E: EditorImplementation>(
        description: String,
    ) -> Self {
        env_logger::builder().format_timestamp_millis().init();

        log::info!("Loader {description} startup");

        let mut dbus_fd_str = None;
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--dbus-fd" => {
                    dbus_fd_str = args.next();
                }

                _ => {
                    log::warn!("Stopping command line parsing at unknown argument: {arg:?}");
                    break;
                }
            }
        }

        let Some(dbus_fd_str) = dbus_fd_str else {
            log::error!("FD that facilitates the D-Bus connection not specified via --dbus-fd");
            std::process::exit(2);
        };

        let Ok(dbus_fd) = dbus_fd_str.parse::<i32>() else {
            log::error!("FD specified via --dbus-fd is not a valid number: {dbus_fd_str:?}",);
            std::process::exit(2);
        };

        log::debug!("Creating zbus connection to glycin");

        let unix_stream: UnixStream = unsafe { UnixStream::from_raw_fd(dbus_fd) };

        #[cfg(feature = "tokio")]
        let unix_stream =
            tokio::net::UnixStream::from_std(unix_stream).expect("wrapping unix stream works");

        let mut dbus_connection_builder = zbus::connection::Builder::unix_stream(unix_stream)
            .p2p()
            .auth_mechanism(zbus::AuthMechanism::Anonymous);

        let loader_instruction_handler = Loader::<L> {
            image_id: Mutex::new(1),
            loader: Default::default(),
        };

        dbus_connection_builder = dbus_connection_builder
            .serve_at("/org/gnome/glycin", loader_instruction_handler)
            .expect("Failed to setup loader handler");

        if E::USEABLE {
            let editor_instruction_handler = Editor::<E> {
                image_id: Mutex::new(1),
                editor: Default::default(),
            };
            dbus_connection_builder = dbus_connection_builder
                .serve_at("/org/gnome/glycin", editor_instruction_handler)
                .expect("Failed to setup editor handler");
        }

        let dbus_connection = dbus_connection_builder
            .build()
            .await
            .expect("Failed to create private DBus connection");

        log::debug!("D-Bus connection to glycin created");
        DbusServer {
            _dbus_connection: dbus_connection,
        }
    }
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

    libc_eprint("glycin sandbox: Blocked syscall used: ");
    libc_eprint(&name.unwrap_or_else(|| String::from("Unknown Syscall")));
    libc_eprint(" (");
    libc_eprint(&syscall.to_string());
    libc_eprint(")\n");

    unsafe {
        libc::exit(128 + libc::SIGSYS);
    }
}

fn setup_sigsys_handler() {
    let mut mask = nix::sys::signal::SigSet::empty();
    mask.add(nix::sys::signal::Signal::SIGSYS);

    let sigaction = nix::sys::signal::SigAction::new(
        nix::sys::signal::SigHandler::SigAction(sigsys_handler),
        nix::sys::signal::SaFlags::SA_SIGINFO,
        mask,
    );

    unsafe {
        if nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGSYS, &sigaction).is_err() {
            libc_eprint("glycin sandbox: Failed to init syscall failure signal handler");
        }
    };
}

#[allow(dead_code)]
pub extern "C" fn pre_main() {
    setup_sigsys_handler();
}

#[macro_export]
macro_rules! init_main_loader {
    ($loader:path) => {
        /// Init handler for SIGSYS before main() to catch
        #[cfg_attr(target_os = "linux", link_section = ".ctors")]
        static __CTOR: extern "C" fn() = pre_main;

        fn main() {
            $crate::DbusServer::spawn_loader::<$loader>(format!(
                "{} v{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ));
        }
    };
}

#[macro_export]
macro_rules! init_main_loader_editor {
    ($loader:path, $editor:path) => {
        /// Init handler for SIGSYS before main() to catch
        #[cfg_attr(target_os = "linux", link_section = ".ctors")]
        static __CTOR: extern "C" fn() = pre_main;

        fn main() {
            $crate::DbusServer::spawn_loader_editor::<$loader, $editor>(format!(
                "{} v{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ));
        }
    };
}

fn libc_eprint(s: &str) {
    unsafe {
        libc::write(
            libc::STDERR_FILENO,
            s.as_ptr() as *const libc::c_void,
            s.len(),
        );
    }
}
