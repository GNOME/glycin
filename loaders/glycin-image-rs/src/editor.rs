use glycin_utils::{EditorImplementation, EditorOutput};

#[derive(Default)]
pub struct ImgEditor {}

impl EditorImplementation for ImgEditor {
    fn apply(
        &self,
        stream: glycin_utils::UnixStream,
        mime_type: String,
        details: glycin_utils::InitializationDetails,
        operations: glycin_utils::operations::Operations,
    ) -> Result<EditorOutput, glycin_utils::LoaderError> {
        Ok(EditorOutput::bit_changes(&[]))
    }
}
