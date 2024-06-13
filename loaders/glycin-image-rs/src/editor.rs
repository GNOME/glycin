use std::io::Read;

use glycin_utils::*;
use operations::Operation;

#[derive(Default)]
pub struct ImgEditor {}

impl EditorImplementation for ImgEditor {
    fn apply(
        &self,
        mut stream: glycin_utils::UnixStream,
        mime_type: String,
        _details: glycin_utils::InitializationDetails,
        operations: glycin_utils::operations::Operations,
    ) -> Result<SparseEditorOutput, glycin_utils::LoaderError> {
        if mime_type != "image/jpeg" {
            return Err(LoaderError::UnsupportedImageFormat(mime_type.to_string()));
        }

        match operations.operations().first().loading_error()? {
            Operation::Rotate(rotation) => {
                let mut buf = Vec::new();
                stream.read_to_end(&mut buf).internal_error()?;

                let jpeg = gufo_jpeg::Jpeg::new(&buf);
                let mut exif_data = jpeg.exif_data();
                let mut exif_segment = jpeg.exif();

                if let (Some(exif_data), Some(exif_segment)) =
                    (exif_data.next(), exif_segment.next())
                {
                    let mut exif = gufo_exif::internal::ExifRaw::new(exif_data.to_vec());
                    exif.decode().loading_error()?;

                    if let Some(entry) = exif.lookup_entry(gufo_common::field::Orientation) {
                        let pos = exif_segment.data_pos() as usize
                            + entry.value_offset_position() as usize
                            + gufo_jpeg::EXIF_IDENTIFIER_STRING.len();

                        let current_orientation =
                            gufo_common::orientation::Orientation::try_from(buf[pos] as u16)
                                .loading_error()?;

                        let new_rotation = current_orientation.rotate() - *rotation;

                        let new_orientation = gufo_common::orientation::Orientation::new(
                            new_rotation,
                            current_orientation.mirror(),
                        );

                        return Ok(SparseEditorOutput::bit_changes(&[(
                            pos as u64,
                            new_orientation as u8,
                        )]));
                    }
                }
            }
        }

        // TODO: Return an error
        Ok(SparseEditorOutput::bit_changes(&[]))
    }
}
