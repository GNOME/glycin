#!/bin/env python3

import os
import json


def add(d, path):
    binary_name = os.path.basename(path)
    filesize = os.path.getsize(path)
    d[binary_name] = {"file-size": {"value": float(filesize)}}


def main():
    bench = {}

    for entry in os.scandir("/usr/libexec/glycin-loaders/2+/"):
        add(bench, entry.path)

    add(bench, "/usr/bin/glycin-thumbnailer")

    print(json.dumps(bench))


main()
