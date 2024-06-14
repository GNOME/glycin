use std::os::unix::net::UnixStream;
use std::sync::Mutex;

#[test]
#[ignore]
fn dbus_api_stability() {
    // TODO: This seems overly complicated
    blocking::unblock(|| async_io::block_on(start_dbus())).detach();
    check_api_stability("org.gnome.glycin.Loader");
    check_api_stability("org.gnome.glycin.Editor");
}

fn check_api_stability(interface_name: &str) {
    let output = std::process::Command::new("busctl")
        .args([
            "introspect",
            "--user",
            "--xml-interface",
            "org.gnome.glycin.Test",
            "/org/gnome/glycin/test",
        ])
        .output()
        .unwrap();

    let compat_version = glycin::COMPAT_VERSION;
    let current_api =
        std::fs::read_to_string(format!("../docs/{compat_version}+/{interface_name}.xml")).unwrap();

    let s = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
  "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
"#
    .to_string();

    let mut api = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .fold((false, s), |(mut take, mut s), line| {
            if line.contains(interface_name) {
                take = true;
            }

            if take {
                s.push_str(line);
                s.push('\n');
            }

            if line.contains("</interface>") {
                take = false;
            }

            (take, s)
        })
        .1;

    api.push_str("</node>\n");

    if current_api != api {
        eprintln!("{api}");
    }

    assert_eq!(api, current_api);
}

async fn start_dbus() {
    let _connection = zbus::ConnectionBuilder::session()
        .unwrap()
        .name("org.gnome.glycin.Test")
        .unwrap()
        .serve_at("/org/gnome/glycin/test", mock_loader())
        .unwrap()
        .serve_at("/org/gnome/glycin/test", mock_editor())
        .unwrap()
        .build()
        .await
        .unwrap();

    std::future::pending::<()>().await;
}

fn mock_loader() -> glycin_utils::Loader {
    struct MockLoader {}

    impl glycin_utils::LoaderImplementation for MockLoader {
        fn init(
            &self,
            _stream: UnixStream,
            _mime_type: String,
            _details: glycin_utils::InitializationDetails,
        ) -> Result<glycin_utils::ImageInfo, glycin_utils::ProcessError> {
            unimplemented!()
        }
        fn frame(
            &self,
            _frame_request: glycin_utils::FrameRequest,
        ) -> Result<glycin_utils::Frame, glycin_utils::ProcessError> {
            unimplemented!()
        }
    }

    let loader_impl = MockLoader {};

    glycin_utils::Loader {
        loader: Mutex::new(Box::new(loader_impl)),
    }
}

fn mock_editor() -> glycin_utils::Editor {
    struct MockEditor {}

    impl glycin_utils::EditorImplementation for MockEditor {
        fn apply(
            &self,
            _stream: UnixStream,
            _mime_type: String,
            _details: glycin_utils::InitializationDetails,
            _operations: glycin_utils::operations::Operations,
        ) -> Result<glycin_utils::SparseEditorOutput, glycin_utils::ProcessError> {
            unimplemented!()
        }
    }

    let editor_impl = MockEditor {};

    glycin_utils::Editor {
        editor: Mutex::new(Box::new(editor_impl)),
    }
}
