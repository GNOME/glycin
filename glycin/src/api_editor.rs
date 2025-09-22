use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use gio::glib;
use gio::prelude::{IsA, *};
use glycin_common::BinaryData;
use glycin_utils::safe_math::SafeConversion;
use glycin_utils::{ByteChanges, CompleteEditorOutput, Operations, SparseEditorOutput};
use zbus::zvariant::OwnedObjectPath;

use crate::api_common::*;
use crate::dbus::EditorProxy;
use crate::error::ResultExt;
use crate::pool::{Pool, PooledProcess};
use crate::util::spawn_detached;
use crate::{config, util, Error, ErrorCtx, MimeType};

/// Image edit builder
#[derive(Debug)]
pub struct Editor {
    source: Source,
    pool: Arc<Pool>,
    cancellable: gio::Cancellable,
    pub(crate) sandbox_selector: SandboxSelector,
}

static_assertions::assert_impl_all!(Editor: Send, Sync);

impl Editor {
    /// Create an editor.
    pub fn new(file: gio::File) -> Self {
        Self {
            source: Source::File(file),
            pool: Pool::global(),
            cancellable: gio::Cancellable::new(),
            sandbox_selector: SandboxSelector::default(),
        }
    }

    pub async fn edit(mut self) -> Result<EditableImage, ErrorCtx> {
        let source: Source = self.source.send();

        let process_context = spin_up_editor(
            source,
            self.pool.clone(),
            &self.cancellable,
            &self.sandbox_selector,
        )
        .await
        .err_no_context(&self.cancellable)?;

        let process = process_context.process.use_();

        let editable_image = process
            .edit(
                &process_context.g_file_worker.unwrap(),
                &process_context.mime_type,
            )
            .await
            .err_context(&process, &self.cancellable)?;

        self.cancellable.connect_cancelled(glib::clone!(
            #[strong(rename_to=process)]
            process_context.process,
            #[strong(rename_to=path)]
            editable_image.edit_request,
            move |_| {
                tracing::debug!("Terminating loader");
                crate::util::spawn_detached(process.use_().done(path))
            }
        ));

        Ok(EditableImage {
            _active_sandbox_mechanism: process_context.sandbox_mechanism,
            editor: self,
            editor_alive: Default::default(),
            edit_request: editable_image.edit_request,
            _mime_type: process_context.mime_type,
            process: process_context.process,
        })
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
    pub(crate) process: Arc<PooledProcess<EditorProxy<'static>>>,
    edit_request: OwnedObjectPath,
    // TODO: Use in error messages
    _mime_type: MimeType,
    _active_sandbox_mechanism: SandboxMechanism,
    editor_alive: Mutex<Arc<()>>,
}

impl Drop for EditableImage {
    fn drop(&mut self) {
        self.process.use_().done_background(self);
        *self.editor_alive.lock().unwrap() = Arc::new(());
        spawn_detached(self.editor.pool.clone().clean_loaders());
    }
}

impl EditableImage {
    /// Apply operations to the image with a potentially sparse result.
    ///
    /// Some operations like rotation can be in some cases be conducted by only
    /// changing one or a few bytes in a file. We call these cases *sparse* and
    /// a [`SparseEdit::Sparse`] is returned.
    pub async fn apply_sparse(self, operations: &Operations) -> Result<SparseEdit, ErrorCtx> {
        let process = self.process.use_();

        let editor_output = process
            .editor_apply_sparse(operations, &self)
            .await
            .err_context(&process, &self.editor.cancellable)?;

        SparseEdit::try_from(editor_output).err_no_context(&self.editor.cancellable)
    }

    /// Apply operations to the image
    pub async fn apply_complete(self, operations: &Operations) -> Result<Edit, ErrorCtx> {
        let process = self.process.use_();

        let editor_output = process
            .editor_apply_complete(operations, &self)
            .await
            .err_context(&process, &self.editor.cancellable)?;

        Ok(Edit {
            inner: editor_output,
        })
    }

    /// List all configured image editors
    pub async fn supported_formats() -> BTreeMap<MimeType, config::ImageEditorConfig> {
        let config = config::Config::cached().await;
        config.image_editor.clone()
    }

    pub(crate) fn edit_request_path(&self) -> OwnedObjectPath {
        self.edit_request.clone()
    }
}

#[derive(Debug)]
/// An image change that is potentially sparse.
///
/// See also: [`Editor::apply_sparse()`]
pub enum SparseEdit {
    /// The operations can be applied to the image via only changing a few
    /// bytes. The [`apply_to()`](Self::apply_to()) function can be used to
    /// apply these changes.
    Sparse(ByteChanges),
    /// The operations require to completely rewrite the image.
    Complete(BinaryData),
}

#[derive(Debug)]
pub struct Edit {
    inner: CompleteEditorOutput,
}

impl Edit {
    pub fn data(&self) -> BinaryData {
        self.inner.data.clone()
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

impl TryFrom<SparseEditorOutput> for SparseEdit {
    type Error = Error;

    fn try_from(value: SparseEditorOutput) -> std::result::Result<Self, Self::Error> {
        if value.byte_changes.is_some() && value.data.is_some() {
            Err(Error::RemoteError(
                glycin_utils::RemoteError::InternalLoaderError(
                    "Sparse editor output with 'byte_changes' and 'data' returned.".into(),
                ),
            ))
        } else if let Some(bit_changes) = value.byte_changes {
            Ok(Self::Sparse(bit_changes))
        } else if let Some(data) = value.data {
            Ok(Self::Complete(data))
        } else {
            Err(Error::RemoteError(
                glycin_utils::RemoteError::InternalLoaderError(
                    "Sparse editor output with neither 'bit_changes' nor 'data' returned.".into(),
                ),
            ))
        }
    }
}
