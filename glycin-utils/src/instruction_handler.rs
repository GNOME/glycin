// Copyright (c) 2024 GNOME Foundation Inc.

use std::os::fd::{AsRawFd, FromRawFd};
use std::os::raw::{c_int, c_void};
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

use nix::libc::{c_uint, siginfo_t};

use crate::dbus_editor_api::{void_editor_none, Editor, EditorImplementation};
use crate::dbus_loader_api::{Loader, LoaderImplementation};

pub struct Communication {
    _dbus_connection: zbus::Connection,
}

impl Communication {
    pub fn spawn_loader(decoder: impl LoaderImplementation + 'static) {
        futures_lite::future::block_on(async move {
            let _connection = Self::connect(Some(decoder), void_editor_none()).await;
            std::future::pending::<()>().await;
        })
    }

    pub fn spawn_loader_editor(
        loader: impl LoaderImplementation + 'static,
        editor: impl EditorImplementation + 'static,
    ) {
        futures_lite::future::block_on(async move {
            let _connection = Self::connect(Some(loader), Some(editor)).await;
            std::future::pending::<()>().await;
        })
    }

    async fn connect(
        loader: Option<impl LoaderImplementation + 'static>,
        editor: Option<impl EditorImplementation + 'static>,
    ) -> Self {
        let unix_stream: UnixStream =
            unsafe { UnixStream::from_raw_fd(std::io::stdin().as_raw_fd()) };

        #[cfg(feature = "tokio")]
        let unix_stream =
            tokio::net::UnixStream::from_std(unix_stream).expect("wrapping unix stream works");

        let mut dbus_connection = zbus::connection::Builder::unix_stream(unix_stream)
            .p2p()
            .auth_mechanism(zbus::AuthMechanism::Anonymous);

        if let Some(loader) = loader {
            let instruction_handler = Loader {
                loader: Mutex::new(Box::new(loader)),
            };
            dbus_connection = dbus_connection
                .serve_at("/org/gnome/glycin", instruction_handler)
                .expect("Failed to setup loader handler");
        }

        if let Some(editor) = editor {
            let instruction_handler = Editor {
                editor: Mutex::new(Box::new(editor)),
            };
            dbus_connection = dbus_connection
                .serve_at("/org/gnome/glycin", instruction_handler)
                .expect("Failed to setup editor handler");
        }

        Communication {
            _dbus_connection: dbus_connection
                .build()
                .await
                .expect("Failed to create private DBus connection"),
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
            if nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGSYS, &sigaction).is_err() {
                libc_eprint("glycin sandbox: Failed to init syscall failure signal handler");
            }
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

        libc_eprint("glycin sandbox: Blocked syscall used: ");
        libc_eprint(&name.unwrap_or_else(|| String::from("Unknown Syscall")));
        libc_eprint(" (");
        libc_eprint(&syscall.to_string());
        libc_eprint(")\n");

        unsafe {
            libc::exit(128 + libc::SIGSYS);
        }
    }
}

#[allow(dead_code)]
pub extern "C" fn pre_main() {
    Communication::setup_sigsys_handler();
}

#[macro_export]
macro_rules! init_main_loader {
    ($loader:expr) => {
        /// Init handler for SIGSYS before main() to catch
        #[cfg_attr(target_os = "linux", link_section = ".ctors")]
        static __CTOR: extern "C" fn() = pre_main;

        fn main() {
            $crate::Communication::spawn_loader($loader);
        }
    };
}

#[macro_export]
macro_rules! init_main_loader_editor {
    ($loader:expr, $editor:expr) => {
        /// Init handler for SIGSYS before main() to catch
        #[cfg_attr(target_os = "linux", link_section = ".ctors")]
        static __CTOR: extern "C" fn() = pre_main;

        fn main() {
            $crate::Communication::spawn_loader_editor($loader, $editor);
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
