import gi
from utils import *

gi.require_version("Gly", "2")
gi.require_version("GlyGtk4", "2")

from gi.repository import Gly


def test_encoding_progressive():
    creator = Gly.Creator(mime_type="image/png")

    data = [0, 1, 2]
    frame = creator.add_frame(1, 1, Gly.MemoryFormat.R8G8B8, GLib.Bytes.new(data))
    frame.set_encoding_progressive(True)

    encoded_image = creator.create().get_data().get_data()
    buf = helper_buf_from_data(encoded_image)

    assert list(buf.get_data()) == data
