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


@pytest.mark.skipif(
    os.name != "posix",
    reason="ulimit requires unix like system",
)
def test_check_fd_leaks():
    resource.setrlimit(resource.RLIMIT_NOFILE, (100, 100))

    file = helper_image_file("images/tiny/tiny.png")

    for i in range(150):
        loader = Gly.Loader(file=file)
        # Force a format transformation since this spawns blocking threads which
        # have caused memory leaks in the past.
        # See <https://gitlab.gnome.org/GNOME/glycin/-/work_items/314>
        loader.set_accepted_memory_formats(Gly.MemoryFormatSelection.A8B8G8R8)
        image = loader.load()
        frame = image.next_frame()
        assert frame is not None


def test_cancellable():
    file = helper_image_file("images/color/color.jpg")

    loader = Gly.Loader(file=file)
    image = loader.load()
    cancellable = Gio.Cancellable()

    image.next_frame_async(cancellable, lambda x: x)
    # This panicked in the past.
    # See <https://gitlab.gnome.org/GNOME/glycin/-/merge_requests/442>
    cancellable.cancel()


def test_gtask_starvation():
    """Test sync libglycin API while blocking the complete GTask pool.

    This tests the `load_with_sync` variants of the internal glycin API.
    """

    class GTaskTest:
        def __init__(self):
            self.success = False
            self.loop = GLib.MainLoop.new(None, False)

        def _thread_func(self, task, source_obj, task_data, cancellable):
            file = helper_image_file("images/color/color.jpg")
            loader = Gly.Loader.new(file)

            image = loader.load()

            task.return_boolean(image is not None)

        def _on_task_complete(self, source_obj, result, user_data):
            self.success = result.propagate_boolean()

            self.loop.quit()

        def run(self):
            for _ in range(100):
                task = Gio.Task.new(None, None, self._on_task_complete, None)
                task.run_in_thread(self._thread_func)

            # Trigger result check after 2 seconds
            GLib.timeout_add_seconds(2, self.loop.quit)

            self.loop.run()

            assert self.success

    GTaskTest().run()
