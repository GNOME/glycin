#!/usr/bin/python3

import tomllib
import sys
import os.path

crate = sys.argv[1]
gnome_version = False if len(sys.argv) < 3 else sys.argv[2] == 'gnome'

path = os.path.dirname(__file__)
data = tomllib.load(open(f'{path}/../{crate}/Cargo.toml', 'rb'))
version = data['package']['version']

if gnome_version:
     if '-' in version:
        (maj, min, patch) = version.split('.', 2)
        patch = patch.split('-', 1)[1]
        version = f'{maj}.{min}.{patch}'

print(version, end='')
