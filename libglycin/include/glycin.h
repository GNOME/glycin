#pragma once

#include <glib-object.h>
#include <gio/gio.h>

G_BEGIN_DECLS

/**
 * GlyLoader:
 *
 * [class@Loader] prepares loading an image.
 *
 * The following example shows how to obtain a [class@Gdk.Texture]. It uses
 * [GlyGtk4](https://sophie-h.pages.gitlab.gnome.org/glycin/libglycin-gtk4)
 * for this.
 *
 * ```c
 * #include <glycin-gtk4.h>
 *
 * file = g_file_new_for_path ("test.png");
 * loader = gly_loader_new (file);
 * image = gly_loader_load (loader, NULL);
 * if (image)
 * {
 *   frame = gly_image_next_frame (image, NULL);
 *   if (frame) {
 *     texture = gly_gtk_frame_get_texture (frame);
 *     printf ("Image height: %d\n", gdk_texture_get_height (texture));
 *     image_widget = gtk_image_new_from_paintable (GDK_PAINTABLE (texture));
 *   }
 * }
 * ```
 *
 * Since: 1.0
 */
#define GLY_TYPE_LOADER (gly_loader_get_type())
G_DECLARE_FINAL_TYPE(GlyLoader, gly_loader, GLY, LOADER, GObject)

/**
 * GlyImage:
 *
 * Image handle containing metadata and allowing frame requests.
 *
 * Since: 1.0
 */
#define GLY_TYPE_IMAGE (gly_image_get_type())
G_DECLARE_FINAL_TYPE(GlyImage, gly_image, GLY, IMAGE, GObject)

/**
 * GlyFrame:
 *
 * A frame of an image often being the complete image.
 *
 * Since: 1.0
 */
#define GLY_TYPE_FRAME (gly_frame_get_type())
G_DECLARE_FINAL_TYPE(GlyFrame, gly_frame, GLY, FRAME, GObject)

/**************** GlySandboxSelector ****************/

/**
 * GlySandboxSelector:
 * @GLY_SANDBOX_SELECTOR_AUTO:
 *  This mode selects `bwrap` outside of Flatpaks and usually
 *  `flatpak-spawn` inside of Flatpaks. The sandbox is disabled
 *  automatically inside of Flatpak development environments.
 *  Inside of Flatpaks, `flatpak-spawn` is used to create the sandbox. This
 *  mechanism starts an installed Flatpak with the same app id. For
 *  development, Flatpak are usually not installed and the sandbox can
 *  therefore not be used. If the sandbox has been started via
 *  `flatpak-builder --run` (i.e. without installed Flatpak) and the app id
 *  ends with `.Devel`, the sandbox is disabled.
 * @GLY_SANDBOX_SELECTOR_BWRAP: bwrap
 * @GLY_SANDBOX_SELECTOR_FLATPAK_SPAWN: flatpak-spawn
 * @GLY_SANDBOX_SELECTOR_NOT_SANDBOXED: Disable sandbox. Unsafe, only use for testing and development.
 *
 * Sandbox mechanisms
 *
 * ::: warning
    Using @GLY_SANDBOX_SELECTOR_NOT_SANDBOXED will disable an important security layer that sandboxes loaders. It is only intended for testing and development purposes.
 * Since: 1.0
 */
typedef enum
{
    GLY_SANDBOX_SELECTOR_AUTO,
    GLY_SANDBOX_SELECTOR_BWRAP,
    GLY_SANDBOX_SELECTOR_FLATPAK_SPAWN,
    GLY_SANDBOX_SELECTOR_NOT_SANDBOXED,
} GlySandboxSelector;

GType gly_sandbox_selector_get_type(void);

/**************** GlyLoader ****************/

/**
 * gly_loader_new:
 * @file: A [iface@Gio.File] from which to load
 *
 * Creates a new [class@Loader].
 *
 * Returns: (transfer full): a new [class@Loader]
 *
 * Since: 1.0
 */
GlyLoader *gly_loader_new(GFile *file);

/**
 * gly_loader_set_sandbox_selector:
 * @loader:
 * @sandbox_selector: Method by which the sandbox mechanism is selected
 *
 * Selects which sandbox mechanism should be used. The default without calling this function is [enum@SandboxSelector]`.AUTO`.
 *
 * Since: 1.0
 */
void gly_loader_set_sandbox_selector(GlyLoader *loader,
                                     GlySandboxSelector sandbox_selector);

/**
 * gly_loader_load:
 * @loader:
 * @error:
 *
 * Synchronously loads an image and returns an [class@Image] when successful.
 *
 * Returns: (transfer full): a new [class@Image] on success, or `NULL` with @error filled in
 *
 * Since: 1.0
 */
GlyImage *gly_loader_load(GlyLoader *loader,
                          GError **error);

/**
 * gly_loader_load_async:
 * @loader:
 * @cancellable: (nullable): A [class@Gio.Cancellable] to cancel the operation
 * @callback: (scope async): A callback to call when the operation is complete
 * @user_data: (closure callback): Data to pass to @callback
 *
 * Asynchronous version of [method@Loader.load].
 *
 * Since: 1.0
 */
void gly_loader_load_async(GlyLoader *loader,
                           GCancellable *cancellable,
                           GAsyncReadyCallback callback,
                           gpointer user_data);

/**
 * gly_loader_load_finish:
 * @loader:
 * @result: A `GAsyncResult`
 * @error:
 *
 * Finishes the [method@Image.next_frame_async] call.
 *
 * Returns: (transfer full): Loaded frame.
 *
 * Since: 1.0
 */
GlyImage *gly_loader_load_finish(GlyLoader *loader,
                                 GAsyncResult *result,
                                 GError **error);

/**************** GlyImage ****************/

/**
 * gly_image_next_frame:
 * @image:
 * @error:
 *
 * Synchronously loads texture and information of the next frame.
 *
 * For single still images, this can only be called once.
 * For animated images, this function will loop to the first frame, when the last frame is reached.
 *
 * Returns: (transfer full): a new [class@Frame] on success, or `NULL` with @error filled in
 *
 * Since: 1.0
 */
GlyFrame *gly_image_next_frame(GlyImage *image,
                               GError **error);

/**
 * gly_image_next_frame_async:
 * @image:
 * @cancellable: (nullable): A [class@Gio.Cancellable] to cancel the operation
 * @callback: (scope async): A callback to call when the operation is complete
 * @user_data: (closure callback): Data to pass to @callback
 *
 * Asynchronous version of [method@Image.next_frame].
 *
 * Since: 1.0
 */
void gly_image_next_frame_async(GlyImage *image,
                                GCancellable *cancellable,
                                GAsyncReadyCallback callback,
                                gpointer user_data);

/**
 * gly_image_next_frame_finish:
 * @image:
 * @result: a `GAsyncResult`
 * @error:
 *
 * Finishes the [method@Image.next_frame_async] call.
 *
 * Returns: (transfer full): Loaded frame.
 *
 * Since: 1.0
 */
GlyFrame *gly_image_next_frame_finish(GlyImage *image,
                                      GAsyncResult *result,
                                      GError **error);

/**
 * gly_image_get_mime_type:
 * @image:
 *
 * Returns detected MIME type of the file
 *
 * Returns: MIME type
 *
 * Since: 1.0
 */
const char *gly_image_get_mime_type(GlyImage *image);

/**
 * gly_image_get_width:
 * @image:
 *
 * Early width information.
 *
 * This information is often correct. However, it should only be used for
 * an early rendering estimates. For everything else, the specific frame
 * information should be used. See [method@Frame.get_width].
 *
 * Returns: Width
 *
 * Since: 1.0
 */
uint32_t gly_image_get_width(GlyImage *image);

/**
 * gly_image_get_height:
 * @image:
 *
 * See [method@Image.get_width]
 *
 * Returns: height
 *
 * Since: 1.0
 */
uint32_t gly_image_get_height(GlyImage *image);

/**************** GlyFrame ****************/

/**
 * GlyMemoryFormat:
 * @GLY_MEMORY_B8G8R8A8_PREMULTIPLIED: 8-bit RGRA premultiplied
 * @GLY_MEMORY_A8R8G8B8_PREMULTIPLIED: 8-bit ARGB premultiplied
 * @GLY_MEMORY_R8G8B8A8_PREMULTIPLIED: 8-bit RGBA premultiplied
 * @GLY_MEMORY_B8G8R8A8: 8-bit RGBA
 * @GLY_MEMORY_A8R8G8B8: 8-bit AGBR
 * @GLY_MEMORY_R8G8B8A8: 8-bit RGBA
 * @GLY_MEMORY_A8B8G8R8: 8-bit ABGR
 * @GLY_MEMORY_R8G8B8: 8-bit RGB
 * @GLY_MEMORY_B8G8R8: 8-bit BGR
 * @GLY_MEMORY_R16G16B16: 16-bit RGB
 * @GLY_MEMORY_R16G16B16A16_PREMULTIPLIED: 16-bit RGBA premultiplied
 * @GLY_MEMORY_R16G16B16A16: 16-bit RGBA
 * @GLY_MEMORY_R16G16B16_FLOAT: 16-bit float RGB
 * @GLY_MEMORY_R16G16B16A16_FLOAT: 16-bit float RGBA
 * @GLY_MEMORY_R32G32B32_FLOAT: 32-bit float RGB
 * @GLY_MEMORY_R32G32B32A32_FLOAT_PREMULTIPLIED: 32-bit float RGBA premultiplied
 * @GLY_MEMORY_R32G32B32A32_FLOAT: 16-bit float RGBA
 * @GLY_MEMORY_G8A8_PREMULTIPLIED: 8-bit gray with alpha premultiplied
 * @GLY_MEMORY_G8A8: 8-bit gray with alpha
 * @GLY_MEMORY_G8: 8-bit gray
 * @GLY_MEMORY_G16A16_PREMULTIPLIED: 16-bit gray with alpha premultiplied
 * @GLY_MEMORY_G16A16: 16-bit gray with alpha
 * @GLY_MEMORY_G16: 16-bit gray
 *
 * Memory format
 *
 * Since: 1.0
 */
typedef enum
{
    GLY_MEMORY_B8G8R8A8_PREMULTIPLIED,
    GLY_MEMORY_A8R8G8B8_PREMULTIPLIED,
    GLY_MEMORY_R8G8B8A8_PREMULTIPLIED,
    GLY_MEMORY_B8G8R8A8,
    GLY_MEMORY_A8R8G8B8,
    GLY_MEMORY_R8G8B8A8,
    GLY_MEMORY_A8B8G8R8,
    GLY_MEMORY_R8G8B8,
    GLY_MEMORY_B8G8R8,
    GLY_MEMORY_R16G16B16,
    GLY_MEMORY_R16G16B16A16_PREMULTIPLIED,
    GLY_MEMORY_R16G16B16A16,
    GLY_MEMORY_R16G16B16_FLOAT,
    GLY_MEMORY_R16G16B16A16_FLOAT,
    GLY_MEMORY_R32G32B32_FLOAT,
    GLY_MEMORY_R32G32B32A32_FLOAT_PREMULTIPLIED,
    GLY_MEMORY_R32G32B32A32_FLOAT,
    GLY_MEMORY_G8A8_PREMULTIPLIED,
    GLY_MEMORY_G8A8,
    GLY_MEMORY_G8,
    GLY_MEMORY_G16A16_PREMULTIPLIED,
    GLY_MEMORY_G16A16,
    GLY_MEMORY_G16,
} GlyMemoryFormat;

GType gly_memory_format_get_type(void);

/**
 * gly_memory_format_has_alpha:
 * @memory_format:
 *
 * Whether a memory format has an alpha channel
 *
 * Returns: Returns `TRUE` if the memory format has an alpha channel
 *
 * Since: 1.0
 */
gboolean gly_memory_format_has_alpha(GlyMemoryFormat memory_format);

/**
 * gly_memory_format_is_premultiplied:
 * @memory_format:
 *
 * Whether a memory format as an alpha channel an the color values are
 * premultiplied with the alpha value
 *
 * Returns: Returns `TRUE` if color channels are premultiplied
 *
 * Since: 1.0
 */
gboolean gly_memory_format_is_premultiplied(GlyMemoryFormat memory_format);

/**
 * gly_frame_get_delay:
 * @frame:
 *
 * Duration to show frame for animations.
 *
 * If the value is zero, the image is not animated.
 *
 * Returns: Duration in microseconds.
 *
 * Since: 1.0
 */
int64_t gly_frame_get_delay(GlyFrame *frame);

/**
 * gly_frame_get_width:
 * @frame:
 *
 * Width for image data in pixels
 *
 * Returns: Width in pixels
 *
 * Since: 1.0
 */
uint32_t gly_frame_get_width(GlyFrame *frame);

/**
 * gly_frame_get_height:
 * @frame:
 *
 * Height for image data in pixels
 *
 * Returns: Height in pixels
 *
 * Since: 1.0
 */
uint32_t gly_frame_get_height(GlyFrame *frame);

/**
 * gly_frame_get_stride:
 * @frame:
 *
 * Width of a row for image data in bytes
 *
 * Returns: Row stride in bytes
 *
 * Since: 1.0
 */
uint32_t gly_frame_get_stride(GlyFrame *frame);

/**
 * gly_frame_get_buf_bytes:
 * @frame:
 *
 * Image data arranged according to [method@Frame.get_memory_format]
 *
 * Returns: (transfer none): Image data
 *
 * Since: 1.0
 */
GBytes *gly_frame_get_buf_bytes(GlyFrame *frame);

/**
 * gly_frame_get_memory_format:
 * @frame:
 *
 * Format of the image data in [method@Gly.Frame.get_buf_bytes]
 *
 * Returns: Format of image data
 *
 * Since: 1.0
 */
GlyMemoryFormat gly_frame_get_memory_format(GlyFrame *frame);

/**************** GlyLoaderError ****************/

/**
 * GlyLoaderError:
 * @GLY_LOADER_ERROR_FAILED: Generic type for all other errors.
 * @GLY_LOADER_ERROR_UNKNOWN_IMAGE_FORMAT: Unknown image format.
 *
 * Errors that can appear while loading images.
 *
 * Since: 1.0
 */
typedef enum
{
    GLY_LOADER_ERROR_FAILED,
    GLY_LOADER_ERROR_UNKNOWN_IMAGE_FORMAT,
} GlyLoaderError;

/**
 * gly_loader_error_quark:
 *
 * Error quark for [error@GlyLoaderError]
 *
 * Returns: The error domain
 */
GQuark gly_loader_error_quark(void) G_GNUC_CONST;

#define GLY_LOADER_ERROR (gly_loader_error_quark())
GType gly_loader_error_get_type(void);

G_END_DECLS
