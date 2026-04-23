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
    pub mime_type: String,
}

impl LoaderImplementation for ImgDecoder {
    fn init<B: ByteData, R: std::io::Read + Send + 'static>(
        mut stream: R,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<(Self, ImageDetails<B>), ProcessError> {
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
            other => panic!("unknwon instruction {other}"),
        }

        Ok((ImgDecoder { instructions }, ImageDetails::new(1, 1)))
    }

    fn frame<T: ByteData>(
        &mut self,
        frame_request: FrameRequest,
    ) -> Result<Frame<T>, ProcessError> {
        unimplemented!()
    }
}

impl EditorImplementation for ImgEditor {
    fn apply_complete<B: ByteData>(
        &self,
        operations: Operations,
    ) -> Result<CompleteEditorOutput<B>, ProcessError> {
        unimplemented!()
    }

    fn create<B: ByteData>(
        mime_type: String,
        new_image: NewImage<B>,
        encoding_options: EncodingOptions,
    ) -> Result<EncodedImage<B>, ProcessError> {
        unimplemented!()
    }

    fn edit<S: std::io::Read + std::any::Any>(
        stream: S,
        mime_type: String,
        details: InitializationDetails,
    ) -> Result<Self, ProcessError> {
        unimplemented!()
    }
}
