#!/usr/bin/python3

import tomllib

data = tomllib.load(open('glycin/Cargo.toml', 'rb'))

print(data['package']['version'], end='')
