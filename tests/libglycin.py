#!/usr/bin/python3

import gi
import os
import os.path
import sys

gi.require_version("Gly", "1")
gi.require_version("GlyGtk", "1")

from gi.repository import Gly, GlyGtk, Gio, GLib

def main():
    GLib.timeout_add_seconds(interval = 2, function = cb_exit)

    dir = os.path.dirname(os.path.abspath(__file__))

    test_image = os.path.join(dir, "test-images/images/color/color.jpg")
    file = Gio.File.new_for_path(test_image)

    # Sync tests

    loader = Gly.Loader(file=file)
    loader.set_sandbox_selector(Gly.SandboxSelector.AUTO)

    image = loader.load()
    frame = image.next_frame()

    width = frame.get_width()
    height = frame.get_height()
    stride = frame.get_stride()
    first_byte = frame.get_buf_bytes().get_data()[0]
    mime_type = image.get_mime_type()
    memory_format = frame.get_memory_format()

    texture = GlyGtk.frame_get_texture(frame)
    texture_width = texture.get_width()

    assert width == 600, f"Wrong width: {width} px"
    assert height == 400, f"Wrong height: {height} px"
    assert stride == 600 * 3, f"Wrong stride: {stride} px"
    assert first_byte > 50 and first_byte < 70, f"Wrong frist byte: {first_byte}"
    assert mime_type == "image/jpeg", f"Wrong mime type {mime_type}"
    assert memory_format == Gly.MemoryFormat.R8G8B8, f"Wrong memory format: {memory_format}"

    assert not Gly.MemoryFormat.has_alpha(memory_format)
    assert not Gly.MemoryFormat.is_premultiplied(memory_format)

    assert texture_width == 600, f"Wrong texture width: {texture_width} px"

    # Async tests

    loader = Gly.Loader(file=file)
    loader.set_sandbox_selector(Gly.SandboxSelector.AUTO)

    image = loader.load_async(None, loader_cb, "loader_data")

    GLib.MainLoop().run()

def loader_cb(loader, result, user_data):
    assert user_data == "loader_data"
    image = loader.load_finish(result)
    image.next_frame_async(None, image_cb, "image_data")

def image_cb(image, result, user_data):
    assert user_data == "image_data"
    frame = image.next_frame_finish(result)

    assert image.get_mime_type() == "image/jpeg"
    sys.exit(0)

def cb_exit():
    print("Test: Exiting after predefined waiting time.", file=sys.stderr)
    sys.exit(1)

if __name__ == "__main__":
    main()