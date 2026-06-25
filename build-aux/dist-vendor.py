#!/usr/bin/env python3

import os
import sys
import subprocess
import tomlkit
import crates_version

os.chdir(os.environ["MESON_PROJECT_DIST_ROOT"])

VERSION = crates_version.get_version("glycin", False)

REMOVE_CRATES = [
    ("glycin", "glycin"),
    ("glycin/glycin-builtin", "glycin-builtin"),
    ("glycin/glycin-external", "glycin-external"),
    ("glycin-common", "glycin-common"),
    ("glycin-core", "glycin-core"),
    ("glycin-utils", "glycin-utils"),
    ("libglycin-rebind/libglycin-rebind", None),
    ("libglycin-rebind/libglycin-rebind/sys", None),
    ("libglycin-rebind/libglycin-rebind-gtk4", None),
    ("libglycin-rebind/libglycin-rebind-gtk4/sys", None),
]

with open("Cargo.toml", "r") as f:
    config = tomlkit.load(f)
    config["workspace"]["members"] = list(
        filter(
            lambda x: x not in map(lambda x: x[0], REMOVE_CRATES),
            config["workspace"]["members"],
        )
    )

    config["workspace"]["default-members"] = list(
        filter(
            lambda x: x not in map(lambda x: x[0], REMOVE_CRATES),
            config["workspace"]["default-members"],
        )
    )

    config["workspace"]["dependencies"]["glycin"].update({"version": VERSION})

    for (path, crate) in REMOVE_CRATES:
        if crate is not None:
            del config["workspace"]["dependencies"][crate]["path"]

with open("Cargo.toml", "w") as f:
    tomlkit.dump(config, f)
