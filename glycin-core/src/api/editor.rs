use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(feature = "builtin")]
use futures_util::FutureExt;
use gio::glib;
use gio::prelude::{IsA, *};
#[cfg(feature = "builtin")]
use glycin_utils::EditorImplementation;
use glycin_utils::safe_math::SafeConversion;
use glycin_utils::{
    ByteChanges, ByteData, CompleteEditorOutput, FungibleMemory, Operations, SparseEditorOutput,
};
#[cfg(feature = "external")]
use zbus::zvariant::OwnedObjectPath;

use crate::api::*;
#[cfg(feature = "external")]
use crate::dbus::EditorProxy;
use crate::error::ResultExt;
use crate::main_context::{MainContextSelector, ProvidesMainContext};
#[cfg(feature = "external")]
use crate::pool::PooledProcess;
use crate::util::{self, CancellableFuture, ShortcutErrorFuture};
use crate::{Error, ErrorKind, MimeType, Pool, config};

/// Image edit builder
#[derive(Debug)]
pub struct Editor {
    source: Source,
    pool: Arc<Pool>,
    cancellable: gio::Cancellable,
    pub(crate) sandbox_selector: SandboxSelector,
    pub(crate) main_context_selector: MainContextSelector,
}

static_assertions::assert_impl_all!(Editor: Send, Sync);

impl Editor {
    /// Create an editor with a [`gio::File`] as source
    pub fn new(file: gio::File) -> Self {
        Self::new_source(Source::File(file))
    }

    /// Create an editor with a [`gio::InputStream`] as source
    ///
    /// # Safety
    ///
    /// The provided stream must no longer be used after being passed to glycin.
    pub unsafe fn new_stream(stream: impl IsA<gio::InputStream>) -> Self {
        unsafe { Self::new_source(Source::Stream(GInputStreamSend::new(stream.upcast()))) }
    }

    /// Create an editor with [`glib::Bytes`] as source
    pub fn new_bytes(bytes: glib::Bytes) -> Self {
        let stream = gio::MemoryInputStream::from_bytes(&bytes);
        unsafe { Self::new_stream(stream) }
    }

    /// Create an editor with [`Vec<u8>`] as source
    pub fn new_vec(buf: Vec<u8>) -> Self {
        let bytes = glib::Bytes::from_owned(buf);
        Self::new_bytes(bytes)
    }

    pub(crate) fn new_source(source: Source) -> Self {
        Self {
            source,
            pool: Pool::global(),
            cancellable: gio::Cancellable::new(),
            sandbox_selector: SandboxSelector::default(),
            main_context_selector: MainContextSelector::Auto,
        }
    }

    pub fn main_context_selector(&mut self, selector: MainContextSelector) -> &mut Self {
        self.main_context_selector = selector;
        self
    }

    pub fn edit(self) -> Pin<Box<dyn Future<Output = Result<EditableImage, Error>> + Send>> {
        Box::pin(async move {
            let main_context = self.main_context();
            let cancellable = self.cancellable.clone();

            let f = || async move { self.edit_internal().await }.make_cancellable(cancellable);

            main_context.spawn_from_within(f).await?
        })
    }

    async fn edit_internal(mut self) -> Result<EditableImage, Error> {
        let source: Source = self.source.send();

        let editor_context = ProcessorContext::new(source, false, &self.sandbox_selector).await?;

        let editor = editor_context
            .editor(self.pool.clone(), &self.cancellable)
            .await?;

        match editor {
            #[cfg(feature = "external")]
            Processor::Binary(editor) => {
                let process = editor.process.use_();

                let (external_reader, load_image_future) =
                    editor.source_transmission.spawn_external()?;

                let editable_image_future = process.edit(external_reader, &editor.mime_type);

                let editable_image = editable_image_future
                    .join_abort_on_error(load_image_future)
                    .await
                    .err_context(&process)?;

                self.cancellable.connect_cancelled(glib::clone!(
                    #[strong(rename_to=process)]
                    editor.process,
                    #[strong(rename_to=path)]
                    editable_image.edit_request,
                    move |_| {
                        tracing::debug!("Terminating loader");
                        util::spawn_detached(process.use_().done(path))
                    }
                ));

                Ok(EditableImage {
                    editor: self,
                    image_editor: ImageEditor::External(ImageEditorExternal {
                        _active_sandbox_mechanism: editor.sandbox_mechanism,
                        process: editor.process,
                        editor_alive: Default::default(),
                        edit_request: editable_image.edit_request,
                    }),
                    _mime_type: editor.mime_type,
                })
            }
            #[cfg(feature = "builtin")]
            Processor::Builtin(builtin) => {
                let mime_type = builtin.mime_type.to_string();
                let details = glycin_utils::InitializationDetails::default();
                let edit_function: Box<dyn FnOnce() -> _ + Send>;

                let (reader, read_data_future) = builtin.source_transmission.spawn_builtin();

                match builtin.builtin {
                    #[cfg(feature = "builtin-image-rs")]
                    config::BuiltinProcessor::ImageRs(_) => {
                        edit_function = Box::new(move || {
                            glycin_image_rs::ImgEditor::edit(reader, mime_type, details)
                                .map(|e| ImageEditorBuiltin::ImageRs(Arc::new(e)))
                        });
                    }
                    #[cfg(feature = "builtin-test")]
                    config::BuiltinProcessor::Test(_) => {
                        edit_function = Box::new(move || {
                            glycin_test::ImgEditor::edit(reader, mime_type, details)
                                .map(|e| ImageEditorBuiltin::Test(Arc::new(e)))
                        });
                    }
                }

                let editor_future = gio::spawn_blocking(move || {
                    edit_function().map_err(|err| Error::from(err.into_editor_error()))
                })
                .map(|x| x.map_err(|_| ErrorKind::ThreadPanic.into()));

                let editor = editor_future
                    .join_abort_on_error(read_data_future)
                    .await??;

                Ok(EditableImage {
                    editor: self,
                    image_editor: ImageEditor::Builtin(editor),
                    _mime_type: builtin.mime_type,
                })
            }
        }
    }

    /// Sets the method by which the sandbox mechanism is selected.
    ///
    /// The default without calling this function is [`SandboxSelector::Auto`].
    pub fn sandbox_selector(&mut self, sandbox_selector: SandboxSelector) -> &mut Self {
        self.sandbox_selector = sandbox_selector;
        self
    }

    /// Set [`Cancellable`](gio::Cancellable) to cancel any editing operations.
    pub fn cancellable(&mut self, cancellable: impl IsA<gio::Cancellable>) -> &mut Self {
        self.cancellable = cancellable.upcast();
        self
    }
}

#[derive(Debug)]
pub struct EditableImage {
    pub(crate) editor: Editor,
    image_editor: ImageEditor,
    // TODO: Use in error messages
    _mime_type: MimeType,
}

impl Drop for EditableImage {
    fn drop(&mut self) {
        #[cfg(feature = "external")]
        #[allow(irrefutable_let_patterns)]
        if let ImageEditor::External(editor) = &self.image_editor {
            editor.process.use_().done_background(self);
            *editor.editor_alive.lock().unwrap() = Arc::new(());
            util::spawn_detached(self.editor.pool.clone().clean_loaders());
        }
    }
}

impl EditableImage {
    /// Apply operations to the image with a potentially sparse result.
    ///
    /// Some operations like rotation can be in some cases be conducted by only
    /// changing one or a few bytes in a file. We call these cases *sparse* and
    /// a [`SparseEdit::Sparse`] is returned.
    pub fn apply_sparse(
        self,
        operations: &Operations,
    ) -> Pin<Box<dyn Future<Output = Result<SparseEdit, Error>> + Send>> {
        let operations = operations.to_owned();
        Box::pin(self.apply_sparse_internal(operations))
    }

    async fn apply_sparse_internal(self, operations: Operations) -> Result<SparseEdit, Error> {
        match &self.image_editor {
            #[cfg(feature = "external")]
            ImageEditor::External(editor) => {
                let process = editor.process.use_();

                let mut editor_output = process
                    .editor_apply_sparse(&operations, &self)
                    .await
                    .err_context(&process)?;

                editor_output.final_seal().await?;

                SparseEdit::try_from(editor_output.into_fungible())
            }
            #[cfg(feature = "builtin")]
            ImageEditor::Builtin(editor) => {
                let editor_function: Box<dyn FnOnce() -> _ + Send>;

                match editor {
                    #[cfg(feature = "builtin-image-rs")]
                    ImageEditorBuiltin::ImageRs(editor) => {
                        let editor = editor.clone();
                        editor_function = Box::new(move || editor.apply_sparse(operations));
                    }
                    #[cfg(feature = "builtin-test")]
                    ImageEditorBuiltin::Test(editor) => {
                        let editor = editor.clone();
                        editor_function = Box::new(move || editor.apply_sparse(operations));
                    }
                }

                let editor_output = gio::spawn_blocking(|| {
                    editor_function().map_err(|e| Error::from(e.into_editor_error()))
                })
                .await
                .map_err(|_| ErrorKind::ThreadPanic)??;

                SparseEdit::try_from(editor_output)
            }
        }
    }

    /// Apply operations to the image
    pub fn apply_complete(
        &self,
        operations: &Operations,
    ) -> Pin<Box<dyn Future<Output = Result<Edit, Error>> + Send + '_>> {
        let operations = operations.to_owned();

        Box::pin(self.apply_complete_internal(operations))
    }

    async fn apply_complete_internal(&self, operations: Operations) -> Result<Edit, Error> {
        match &self.image_editor {
            #[cfg(feature = "external")]
            ImageEditor::External(editor) => {
                let process = editor.process.use_();

                let mut editor_output = process
                    .editor_apply_complete(&operations, self)
                    .await
                    .err_context(&process)?
                    .into_fungible();

                editor_output.final_seal().await?;

                Ok(Edit {
                    inner: editor_output,
                })
            }
            #[cfg(feature = "builtin")]
            ImageEditor::Builtin(editor) => {
                let apply_function: Box<dyn FnOnce() -> _ + Send + 'static>;

                match editor {
                    #[cfg(feature = "builtin-image-rs")]
                    ImageEditorBuiltin::ImageRs(editor) => {
                        let editor = editor.clone();
                        apply_function = Box::new(move || editor.apply_complete(operations));
                    }
                    #[cfg(feature = "builtin-test")]
                    ImageEditorBuiltin::Test(editor) => {
                        let editor = editor.clone();
                        apply_function = Box::new(move || editor.apply_complete(operations));
                    }
                }

                let editor_output = gio::spawn_blocking(|| {
                    apply_function().map_err(|e| Error::from(e.into_editor_error()))
                })
                .await
                .map_err(|_| ErrorKind::ThreadPanic)??;

                Ok(Edit {
                    inner: editor_output,
                })
            }
        }
    }

    /// List all configured image editors
    pub async fn supported_formats() -> BTreeMap<MimeType, config::ImageEditorConfig> {
        let config = config::Config::cached().await;
        config.image_editor.clone()
    }

    #[cfg(feature = "external")]
    pub(crate) fn edit_request_path(&self) -> OwnedObjectPath {
        #[allow(irrefutable_let_patterns)]
        if let ImageEditor::External(editor) = &self.image_editor {
            editor.edit_request.clone()
        } else {
            todo!()
        }
    }
}

#[derive(Debug)]
enum ImageEditor {
    #[cfg(feature = "external")]
    External(ImageEditorExternal),
    #[cfg(feature = "builtin")]
    Builtin(ImageEditorBuiltin),
}

#[cfg(feature = "external")]
#[derive(Debug)]
struct ImageEditorExternal {
    pub(crate) process: Arc<PooledProcess<EditorProxy<'static>>>,
    edit_request: OwnedObjectPath,
    _active_sandbox_mechanism: SandboxMechanism,
    editor_alive: std::sync::Mutex<Arc<()>>,
}

#[cfg(feature = "builtin")]
#[derive(Clone)]
enum ImageEditorBuiltin {
    #[cfg(feature = "builtin-image-rs")]
    ImageRs(Arc<glycin_image_rs::ImgEditor>),
    #[cfg(feature = "builtin-test")]
    Test(Arc<glycin_test::ImgEditor>),
}

#[cfg(feature = "builtin")]
impl std::fmt::Debug for ImageEditorBuiltin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImageEditorBuiltin")
    }
}

#[derive(Debug)]
/// An image change that is potentially sparse.
///
/// See also: [`EditableImage::apply_sparse()`]
pub enum SparseEdit {
    /// The operations can be applied to the image via only changing a few
    /// bytes. The [`apply_to()`](Self::apply_to()) function can be used to
    /// apply these changes.
    Sparse(ByteChanges),
    /// The operations require to completely rewrite the image.
    Complete(FungibleMemory),
}

#[derive(Debug)]
pub struct Edit {
    inner: CompleteEditorOutput<FungibleMemory>,
}

impl Edit {
    pub fn data(&self) -> &[u8] {
        &self.inner.data
    }

    pub fn is_lossless(&self) -> bool {
        self.inner.info.lossless
    }
}

#[derive(Debug, PartialEq, Eq)]
#[must_use]
/// Whether an image could be changed via the chosen method.
pub enum EditOutcome {
    Changed,
    Unchanged,
}

impl SparseEdit {
    /// Apply sparse changes if applicable.
    ///
    /// If the type does not carry sparse changes, the function will return an
    /// [`EditOutcome::Unchanged`] and the complete image needs to be rewritten.
    pub async fn apply_to(&self, file: gio::File) -> Result<EditOutcome, Error> {
        match self {
            Self::Sparse(bit_changes) => {
                let bit_changes = bit_changes.clone();
                util::spawn_blocking(move || {
                    let stream = file.open_readwrite(gio::Cancellable::NONE)?;
                    let output_stream = stream.output_stream();
                    for change in bit_changes.changes {
                        stream.seek(
                            change.offset.try_i64()?,
                            glib::SeekType::Set,
                            gio::Cancellable::NONE,
                        )?;
                        let (_, err) =
                            output_stream.write_all(&[change.new_value], gio::Cancellable::NONE)?;

                        if let Some(err) = err {
                            return Err(err.into());
                        }
                    }
                    Ok(EditOutcome::Changed)
                })
                .await
            }
            Self::Complete(_) => Ok(EditOutcome::Unchanged),
        }
    }
}

impl TryFrom<SparseEditorOutput<FungibleMemory>> for SparseEdit {
    type Error = Error;

    fn try_from(
        value: SparseEditorOutput<FungibleMemory>,
    ) -> std::result::Result<Self, Self::Error> {
        if value.byte_changes.is_some() && value.data.is_some() {
            Err(
                ErrorKind::RemoteError(glycin_utils::RemoteError::InternalLoaderError(
                    "Sparse editor output with 'byte_changes' and 'data' returned.".into(),
                ))
                .into(),
            )
        } else if let Some(bit_changes) = value.byte_changes {
            Ok(Self::Sparse(bit_changes))
        } else if let Some(data) = value.data {
            Ok(Self::Complete(data.into_fungible()))
        } else {
            Err(
                ErrorKind::RemoteError(glycin_utils::RemoteError::InternalLoaderError(
                    "Sparse editor output with neither 'bit_changes' nor 'data' returned.".into(),
                ))
                .into(),
            )
        }
    }
}
