#pragma once

#include <glycin.h>
#include <gtk/gtk.h>

G_BEGIN_DECLS

/**
 * gly_gtk_frame_get_texture:
 * @frame: Frame
 *
 * Gets the actual image from a frame. See the [class@Gly.Loader] docs
 * for a complete example.
 *
 * Returns: (transfer full): A GDK Texture
 *
 * Since: 2.0
 */
GdkTexture *gly_gtk_frame_get_texture(GlyFrame *frame);

G_END_DECLS
