use gdk::ffi::GdkTexture;
use gdk::glib;
use glib::subclass::prelude::*;
use glib::translate::*;
use glycin::gobject::{self, GlyCicp};

pub type GlyFrame = <gobject::frame::imp::GlyFrame as ObjectSubclass>::Instance;

extern "C" {
    pub fn gly_frame_get_width(frame: *mut GlyFrame) -> u32;
    pub fn gly_frame_get_height(frame: *mut GlyFrame) -> u32;
    pub fn gly_frame_get_memory_format(frame: *mut GlyFrame) -> i32;
    pub fn gly_frame_get_stride(frame: *mut GlyFrame) -> u32;
    pub fn gly_frame_get_buf_bytes(frame: *mut GlyFrame) -> *mut glib::ffi::GBytes;
    pub fn gly_frame_get_color_cicp(frame: *mut GlyFrame) -> *mut GlyCicp;

    pub fn gly_cicp_free(cicp: *mut GlyCicp);
}

#[no_mangle]
pub unsafe extern "C" fn gly_gtk_frame_get_texture(frame: *mut GlyFrame) -> *mut GdkTexture {
    let width = gly_frame_get_width(frame) as i32;
    let height = gly_frame_get_height(frame) as i32;
    let bytes = gly_frame_get_buf_bytes(frame);
    let stride = gly_frame_get_stride(frame) as usize;

    let cicp = gly_frame_get_color_cicp(frame);

    let color_state = if !cicp.is_null() {
        let gdk_cicp = gdk::CicpParams::new();

        gdk_cicp.set_color_primaries((*cicp).color_primaries as u32);
        gdk_cicp.set_transfer_function((*cicp).transfer_characteristics as u32);
        gdk_cicp.set_matrix_coefficients((*cicp).matrix_coefficients as u32);

        let range = match (*cicp).video_full_range_flag {
            0 => gdk::CicpRange::Narrow,
            _ => gdk::CicpRange::Full,
        };

        gdk_cicp.set_range(range);

        gly_cicp_free(cicp);

        gdk_cicp.build_color_state().ok()
    } else {
        None
    };

    let gly_format = glycin::MemoryFormat::try_from(gly_frame_get_memory_format(frame)).unwrap();
    let gdk_format = glycin::gdk_memory_format(gly_format).into_glib();

    let texture = gdk::ffi::gdk_memory_texture_builder_new();

    gdk::ffi::gdk_memory_texture_builder_set_width(texture, width);
    gdk::ffi::gdk_memory_texture_builder_set_height(texture, height);
    gdk::ffi::gdk_memory_texture_builder_set_format(texture, gdk_format);
    gdk::ffi::gdk_memory_texture_builder_set_bytes(texture, bytes);
    gdk::ffi::gdk_memory_texture_builder_set_stride(texture, stride);

    if let Some(color_state) = color_state {
        gdk::ffi::gdk_memory_texture_builder_set_color_state(texture, color_state.to_glib_none().0);
    }

    let result = gdk::ffi::gdk_memory_texture_builder_build(texture) as *mut GdkTexture;

    glib::gobject_ffi::g_object_unref(texture as *mut _);

    result
}
