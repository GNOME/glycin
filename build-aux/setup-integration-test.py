#!/usr/bin/env python3

"""
Rewrite CONFIG_FILE in DESTDIR such that Exec points into DESTDIR
"""

import os
import re

destdir = os.environ['DESTDIR']
rel_config_file = os.path.relpath(os.environ['CONFIG_FILE'], '/')
config_file = os.path.join(destdir, rel_config_file)

with open(config_file, 'r') as f:
    cfg = f.read()
with open(config_file, 'w') as f:
    new_cfg = re.sub('^Exec=', 'Exec=' + destdir, cfg, flags=re.M)
    f.write(new_cfg)
