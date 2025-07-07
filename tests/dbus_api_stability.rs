use std::os::unix::net::UnixStream;

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
    let _connection = zbus::connection::Builder::session()
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

struct MockLoader {}

impl glycin_utils::LoaderImplementation for MockLoader {
    fn frame(
        &mut self,
        _frame_request: glycin_utils::FrameRequest,
    ) -> Result<glycin_utils::Frame, glycin_utils::ProcessError> {
        unimplemented!()
    }

    fn init(
        _stream: UnixStream,
        _mime_type: String,
        _details: glycin_utils::InitializationDetails,
    ) -> Result<(Self, glycin_utils::ImageDetails), glycin_utils::ProcessError> {
        unimplemented!()
    }
}

fn mock_loader() -> glycin_utils::Loader<MockLoader> {
    glycin_utils::Loader {
        loader: Default::default(),
        image_id: Default::default(),
    }
}

struct MockEditor {}

impl glycin_utils::EditorImplementation for MockEditor {
    fn edit(
        _stream: UnixStream,
        _mime_type: String,
        _details: glycin_utils::InitializationDetails,
    ) -> Result<Self, glycin_utils::ProcessError> {
        unimplemented!()
    }

    fn create(
        _mime_type: String,
        _new_image: glycin_utils::NewImage,
        _encoding_options: glycin_utils::EncodingOptions,
    ) -> Result<glycin_utils::EncodedImage, glycin_utils::ProcessError> {
        unimplemented!()
    }

    fn apply_complete(
        &self,
        _operations: glycin::Operations,
    ) -> Result<glycin_utils::CompleteEditorOutput, glycin_utils::ProcessError> {
        unimplemented!()
    }
}

fn mock_editor() -> glycin_utils::Editor<MockEditor> {
    glycin_utils::Editor {
        editor: Default::default(),
        image_id: Default::default(),
    }
}
