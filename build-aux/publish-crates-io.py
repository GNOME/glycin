#!/usr/bin/env python3

import subprocess
import json
import sys

# Publish crates to crates.io

def release_crate(package_name):
    metadata = subprocess.run(['cargo', 'metadata', '--format-version=1', '--no-deps'], capture_output=True, check=True)
    packages = json.loads(metadata.stdout)['packages']
    package = next(p for p in packages if p['name'] == package_name)
    package_version = package['version']
    metadata = subprocess.run(['curl', '-sw', '%{http_code}', '-o', '/dev/null', f'https://crates.io/api/v1/crates/{package_name}/{package_version}' ], capture_output=True, check=True)
    http_code = metadata.stdout

    if http_code == b'404':
        print(f"Publishing crate {package_name} version {package_version}:", file=sys.stderr)
        subprocess.run(['cargo', 'publish', '-p', package_name], check=True)
    else:
        print(f"Crate {package_name} with version {package_version} already published (http code {http_code}). Skipping.")

release_crate('glycin-common')
release_crate('glycin-utils')
release_crate('glycin')
