#!/usr/bin/python3

import gi
import os
import os.path
import sys

gi.require_version("Gly", "2")
gi.require_version("GlyGtk4", "2")

from gi.repository import Gly, GlyGtk4, Gio, GLib, Gdk

# test loader for color.jpg
def test_loader(loader):
    image = loader.load()
    test_image(image)

def test_image(image):
    mime_type = image.get_mime_type()
    none_existent_value = image.get_metadata_key_value("does-not-exist")

    assert mime_type == "image/jpeg", f"Wrong mime type {mime_type}"
    assert none_existent_value is None

    frame = image.next_frame()
    test_frame(frame)

def test_frame(frame):
    width = frame.get_width()
    height = frame.get_height()
    stride = frame.get_stride()
    first_byte = frame.get_buf_bytes().get_data()[0]
    memory_format = frame.get_memory_format()

    texture = GlyGtk4.frame_get_texture(frame)
    texture_width = texture.get_width()

    assert width == 600, f"Wrong width: {width} px"
    assert height == 400, f"Wrong height: {height} px"
    assert stride == 600 * 3, f"Wrong stride: {stride} px"
    assert first_byte > 50 and first_byte < 70, f"Wrong first byte: {first_byte}"
    assert memory_format == Gly.MemoryFormat.R8G8B8, f"Wrong memory format: {memory_format}"
    assert frame.get_color_cicp() is None

    assert not Gly.MemoryFormat.has_alpha(memory_format)
    assert not Gly.MemoryFormat.is_premultiplied(memory_format)

    assert texture_width == 600, f"Wrong texture width: {texture_width} px"

def main():
    GLib.timeout_add_seconds(interval = 2, function = cb_exit)

    dir = os.path.dirname(os.path.abspath(__file__))

    test_image = os.path.join(dir, "test-images/images/color/color.jpg")
    file = Gio.File.new_for_path(test_image)

    test_image_png = os.path.join(dir, "test-images/images/exif.png")
    file_png = Gio.File.new_for_path(test_image_png)

    test_image_cicp = os.path.join(dir, "test-images/images/cicp-p3/cicp-p3.png")
    file_cicp = Gio.File.new_for_path(test_image_cicp)

    test_image_orientation = os.path.join(dir, "test-images/images/color-exif-orientation/color-rotated-90.jpg")
    file_orientation = Gio.File.new_for_path(test_image_orientation)

    test_image_animation = os.path.join(dir, "test-images/images/animated-numbers/animated-numbers.apng")
    file_animation = Gio.File.new_for_path(test_image_animation)

    # Types

    assert Gly.SandboxSelector.AUTO.__gtype__.name == "GlySandboxSelector"
    assert Gly.MemoryFormat.G8.__gtype__.name == "GlyMemoryFormat"
    assert Gly.MemoryFormatSelection.G8.__gtype__.name == "GlyMemoryFormatSelection"

    # Sync basics

    loader = Gly.Loader(file=file)
    loader.set_sandbox_selector(Gly.SandboxSelector.AUTO)

    test_loader(loader)

    loader = Gly.Loader(file=file)
    loader.set_sandbox_selector(Gly.SandboxSelector.NOT_SANDBOXED)

    test_loader(loader)

    # Loader constructors/sources

    loader = Gly.Loader.new(file)
    image = loader.load()
    frame = image.next_frame()
    assert frame.get_width() == 600

    loader = Gly.Loader.new_for_stream(file.read())
    test_loader(loader)

    loader = Gly.Loader(stream=file.read())
    test_loader(loader)

    with open(test_image, 'rb') as f:
        bytes = GLib.Bytes.new(f.read())

    loader = Gly.Loader.new_for_bytes(bytes)
    test_loader(loader)

    loader = Gly.Loader(bytes=bytes)
    test_loader(loader)

    # Memory selection

    loader = Gly.Loader(file=file)
    loader.set_accepted_memory_formats(Gly.MemoryFormatSelection.G8)

    image = loader.load()
    frame = image.next_frame()

    memory_format = frame.get_memory_format()

    assert memory_format == Gly.MemoryFormat.G8, f"Memory format was not accepted: {memory_format}"

    # Don't apply transformations

    loader = Gly.Loader(file=file_orientation)
    rotated_image = loader.load()

    rotated_frame = rotated_image.next_frame()

    loader = Gly.Loader(file=file_orientation)
    loader.set_apply_transformations(False)
    image = loader.load()

    frame = image.next_frame()

    assert rotated_frame.get_width() == frame.get_height()
    assert rotated_frame.get_height() == frame.get_width()

    assert rotated_image.get_width() == frame.get_height()
    assert rotated_image.get_height() == frame.get_width()

    # Orientation

    loader = Gly.Loader(file=file_orientation)
    image = loader.load()

    assert image.get_transformation_orientation() == 8

    loader = Gly.Loader(file=file)
    image = loader.load()

    assert image.get_transformation_orientation() == 1

    # Metadata: Key-Value

    loader = Gly.Loader.new(file_png)
    image = loader.load()

    key_value_exif_model = image.get_metadata_key_value("exif:Model")
    key_value_empty = image.get_metadata_key_value("does-not-exist")
    key_list = image.get_metadata_keys()

    assert key_value_exif_model == "Canon EOS 400D DIGITAL"
    assert key_value_empty is None
    assert "exif:DateTime" in key_list

    # CICP

    loader = Gly.Loader.new(file_cicp)
    image = loader.load()

    frame = image.next_frame()
    cicp = frame.get_color_cicp()

    assert cicp.color_primaries == 12
    assert cicp.transfer_characteristics == 13
    assert cicp.matrix_coefficients == 0
    assert cicp.video_full_range_flag == 1

    cicp_copy = cicp.copy()

    assert cicp_copy is not cicp
    assert cicp_copy.color_primaries == cicp.color_primaries
    assert cicp_copy.transfer_characteristics == cicp.transfer_characteristics
    assert cicp_copy.matrix_coefficients == cicp.matrix_coefficients
    assert cicp_copy.video_full_range_flag == cicp.video_full_range_flag

    texture = GlyGtk4.frame_get_texture(frame)
    cicp = texture.get_color_state().create_cicp_params()

    assert cicp.get_color_primaries() == 12
    assert cicp.get_transfer_function() == 13
    assert cicp.get_matrix_coefficients() == 0
    assert cicp.get_range() == Gdk.CicpRange.FULL

    # Animation

    loader = Gly.Loader.new(file_animation)
    image = loader.load()
    frame_request = Gly.FrameRequest()
    for i in range(0, 2 * 4):
        image.get_specific_frame(frame_request)

    frame_request.set_loop_animation(False)
    try:
        image.get_specific_frame(frame_request)
    except GLib.Error as e:
        assert (e.matches(Gly.LoaderError.quark(), Gly.LoaderError.NO_MORE_FRAMES))
    else:
        raise Exception('Failed to raise Error')


    # Functions

    assert len(Gly.Loader.get_mime_types()) > 0

    # Creator

    creator = Gly.Creator.new("image/png")
    creator.add_metadata_key_value("key", "Value")
    creator.set_encoding_compression(50)

    data = GLib.Bytes.new([1, 2, 3, 4])
    frame = creator.add_frame_with_stride(1, 1, 4, Gly.MemoryFormat.R8G8B8, data)
    frame.set_color_icc_profile(data)

    encoded_image = creator.create()

    encoded_image_data = encoded_image.get_data().get_data()
    assert list(encoded_image_data[0:4]) == [0x89, 0x50, 0x4E, 0x47]

    loader = Gly.Loader.new_for_bytes(encoded_image.get_data())
    image = loader.load()

    assert list(image.get_metadata_key_value("key")) == list("Value")

    error_domain = None
    try:
        creator = Gly.Creator.new("unknown/mime")
    except GLib.GError as e:
        error_domain = e.domain
    assert error_domain == "gly-loader-error"

    creator = Gly.Creator(mime_type="image/jpeg")
    creator.set_sandbox_selector(Gly.SandboxSelector.NOT_SANDBOXED)

    frame = creator.add_frame_with_stride(1, 1, 4, Gly.MemoryFormat.R8G8B8, data)
    creator.create()

    # Async
    global async_tests_remaining
    async_tests_remaining = 0

    loader = Gly.Loader(file=file)
    image = loader.load()
    frame_request = Gly.FrameRequest()
    frame_request.set_scale(32, 32)
    frame = image.get_specific_frame_async(frame_request, None, specific_frame_cb, None)
    async_tests_remaining += 1

    loader = Gly.Loader(file=file)
    loader.set_sandbox_selector(Gly.SandboxSelector.AUTO)
    image = loader.load_async(None, loader_cb, "loader_data")
    async_tests_remaining += 1

    Gly.Loader.get_mime_types_async(None, mime_types_cb, None)
    async_tests_remaining += 1

    # Async Creator

    creator = Gly.Creator(mime_type="image/jpeg")
    creator.set_encoding_quality(50)

    data = GLib.Bytes.new([1,2,3])
    creator.add_frame(width=1, height=1, memory_format=Gly.MemoryFormat.R8G8B8, texture=data)

    creator.create_async(None, creator_cb, None)
    async_tests_remaining += 1

    # Main loop

    GLib.MainLoop().run()

def loader_cb(loader, result, user_data):
    assert user_data == "loader_data"
    image = loader.load_finish(result)
    image.next_frame_async(None, image_cb, "image_data")

def image_cb(image, result, user_data):
    assert user_data == "image_data"
    frame = image.next_frame_finish(result)

    assert image.get_mime_type() == "image/jpeg"

    test_frame(frame)

    async_test_done()

def mime_types_cb(obj, result, user_data):
    mime_types = Gly.Loader.get_mime_types_finish(result)

    assert obj is None
    assert len(mime_types) > 0

    async_test_done()

def specific_frame_cb(image, result, user_data):
    assert user_data is None
    frame = image.get_specific_frame_finish(result)

    assert image.get_mime_type() == "image/jpeg"

    test_frame(frame)

    async_test_done()

def creator_cb(creator, result, user_data):
    encoded_image = creator.create_finish(result)

    data = encoded_image.get_data().get_data()
    assert list(data[0:4]) == [0xFF, 0xD8, 0xFF, 0xE0]

    async_test_done()

def async_test_done():
    global async_tests_remaining
    async_tests_remaining -= 1
    print("Global tests remaining:", async_tests_remaining)
    if async_tests_remaining == 0:
        sys.exit(0)

def cb_exit():
    print("Test: Exiting after predefined waiting time.", file=sys.stderr)
    sys.exit(1)

if __name__ == "__main__":
    main()
