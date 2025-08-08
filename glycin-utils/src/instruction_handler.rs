// Copyright (c) 2024 GNOME Foundation Inc.

use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;

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

        let args = std::env::args().collect::<Vec<_>>();

        if args.get(1).map(|x| x.as_str()) != Some("--dbus-fd") {
            log::error!("FD that facilitates the D-Bus connection not specified via --dbus-fd");
            std::process::exit(2);
        }

        let Some(fd) = args.get(2).and_then(|x| x.parse().ok()) else {
            eprintln!(
                "FD specified via --dbus-fd is not a valid number: {:?}",
                args.get(2)
            );
            std::process::exit(2);
        };

        log::debug!("Creating zbus connection to glycin");

        let unix_stream: UnixStream = unsafe { UnixStream::from_raw_fd(fd) };

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

#[macro_export]
macro_rules! init_main_loader {
    ($loader:path) => {
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
        fn main() {
            $crate::DbusServer::spawn_loader_editor::<$loader, $editor>(format!(
                "{} v{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ));
        }
    };
}
