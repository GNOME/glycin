use std::io::Cursor;

use glycin_utils::*;

#[cfg(feature = "builtin")]
#[derive(Debug, Clone)]
pub struct BuiltinTest;

#[cfg(feature = "builtin")]
impl Builtin for BuiltinTest {
    fn config(&self) -> &'static str {
        include_str!("../glycin-test.conf")
    }

    fn name(&self) -> &'static str {
        "test"
    }
}

pub struct ImgDecoder {
    pub instructions: Vec<String>,
}

pub struct ImgEditor {
    pub instructions: Vec<String>,
}

fn handle_instructions<B: ByteData>(
    mut stream: impl std::io::Read,
) -> Result<Vec<String>, ProcessError> {
    let mut data = String::new();
    stream.read_to_string(&mut data).unwrap();

    let (_, instruction) = data.split_once('\0').unwrap();

    let instructions = instruction
        .split(':')
        .map(|x| x.to_string())
        .collect::<Vec<_>>();

    match instructions[0].as_str() {
        "panic" => panic!("Ordered to panic"),
        "infinte-loop" => loop {},
        "alloc" => {
            B::new(instructions[1].parse().unwrap()).expected_error()?;
        }
        "panic-next-step" => (),
        other => panic!("unknwon instruction {other}"),
    }

    Ok(instructions)
}

impl LoaderImplementation for ImgDecoder {
    fn init<B: ByteData, R: std::io::Read + Send + 'static>(
        stream: R,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<(Self, ImageDetails<B>), ProcessError> {
        let instructions = handle_instructions::<B>(stream)?;

        Ok((ImgDecoder { instructions }, ImageDetails::new(1, 1)))
    }

    fn frame<T: ByteData>(
        &mut self,
        _frame_request: FrameRequest,
    ) -> Result<Frame<T>, ProcessError> {
        match self.instructions[0].as_str() {
            "panic-next-step" => panic!("Requested frame panic"),
            other => panic!("unknwon instruction {other}"),
        }
    }
}

impl EditorImplementation for ImgEditor {
    fn create<B: ByteData>(
        _mime_type: String,
        new_image: NewImage<B>,
        _encoding_options: EncodingOptions,
    ) -> Result<EncodedImage<B>, ProcessError> {
        handle_instructions::<B>(Cursor::new(new_image.frames[0].texture.to_vec()))?;

        Ok(EncodedImage::new(B::new(1).unwrap()))
    }

    fn edit<S: std::io::Read + std::any::Any>(
        stream: S,
        _mime_type: String,
        _details: InitializationDetails,
    ) -> Result<Self, ProcessError> {
        let instructions = handle_instructions::<LocalMemory>(stream)?;

        Ok(ImgEditor { instructions })
    }

    fn apply_complete<B: ByteData>(
        &self,
        _operations: Operations,
    ) -> Result<CompleteEditorOutput<B>, ProcessError> {
        match self.instructions[0].as_str() {
            "panic-next-step" => panic!("Requested frame panic"),
            other => panic!("unknwon instruction {other}"),
        }
    }
}
