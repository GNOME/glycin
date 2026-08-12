import gi
import os
import resource
import pytest

gi.require_version("Gly", "2")
gi.require_version("GlyGtk4", "2")

from gi.repository import Gly, GlyGtk4, Gio, GLib, Gdk


@pytest.mark.skipif(
    os.name != "posix",
    reason="ulimit requires unix like system",
)
def test_check_fd_leaks():
    resource.setrlimit(resource.RLIMIT_NOFILE, (100, 100))

    current_dir = os.path.dirname(os.path.abspath(__file__))

    test_image = os.path.join(current_dir, "../test-images/images/tiny/tiny.png")
    file = Gio.File.new_for_path(test_image)

    for i in range(150):
        loader = Gly.Loader(file=file)
        # Force a format transformation since this spawns blocking threads which
        # have caused memory leaks in the past.
        # See <https://gitlab.gnome.org/GNOME/glycin/-/work_items/314>
        loader.set_accepted_memory_formats(Gly.MemoryFormatSelection.A8B8G8R8)
        image = loader.load()
        frame = image.next_frame()
        assert frame is not None
