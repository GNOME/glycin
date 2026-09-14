import os
import resource

import gi
import pytest

gi.require_version("Gly", "2")
gi.require_version("GlyGtk4", "2")

from gi.repository import Gly, GlyGtk4, Gio, GLib, Gdk


def helper_image_path(path):
    current_dir = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(current_dir, "../test-images", path)


def helper_image_file(path):
    return Gio.File.new_for_path(helper_image_path(path))


def helper_buf_from_data(data):
    loader = Gly.Loader(bytes=GLib.Bytes.new(data))
    image = loader.load()
    frame = image.next_frame()
    return frame.get_buf_bytes()
