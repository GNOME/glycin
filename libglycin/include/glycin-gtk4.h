#pragma once

#include <glycin.h>

G_BEGIN_DECLS

/**
 * gly_gtk_frame_get_texture:
 * @frame: Frame
 *
 * Gets the actual image from a frame.
 *
 * Returns: (transfer full): A GDK Texture
 *
 * Since: 1.0
 */
GdkTexture *gly_gtk_frame_get_texture(GlyFrame *frame);

G_END_DECLS