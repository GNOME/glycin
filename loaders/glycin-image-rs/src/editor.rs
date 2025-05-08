mod jpeg;
mod png;

use glycin_utils::*;

#[derive(Default)]
pub struct ImgEditor {}

impl EditorImplementation for ImgEditor {
    fn apply_sparse(
        stream: glycin_utils::UnixStream,
        mime_type: String,
        details: glycin_utils::InitializationDetails,
        operations: glycin_utils::operations::Operations,
    ) -> Result<SparseEditorOutput, glycin_utils::ProcessError> {
        match mime_type.as_str() {
            "image/jpeg" => Ok(jpeg::apply_sparse(stream, operations)?),
            _ => Ok(SparseEditorOutput::from(Self::apply_complete(
                stream, mime_type, details, operations,
            )?)),
        }
    }

    fn apply_complete(
        stream: UnixStream,
        mime_type: String,
        _details: InitializationDetails,
        operations: operations::Operations,
    ) -> Result<CompleteEditorOutput, ProcessError> {
        match mime_type.as_str() {
            "image/png" => png::apply(stream, operations),
            "image/jpeg" => Ok(jpeg::apply_complete(stream, operations)?),
            mime_type => Err(ProcessError::UnsupportedImageFormat(mime_type.to_string())),
        }
    }
}
