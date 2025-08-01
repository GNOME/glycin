#!/usr/bin/env python3

import os
import os.path
import sys
import textwrap
import subprocess
import json

release_names = []

BASE_DIR = 'news.d'
OUT_FILE = 'NEWS'
HEADING = ''
IGNORED_PACKAGES = ['tests', 'glycin-dev-tools']

def main():
    changelog = Changelog(BASE_DIR, HEADING)
    changelog.load()

    this_release = changelog.releases[-1]
    last_release = changelog.releases[-2]
    componens = Components(this_release.name, last_release.name)
    componens.write()
    this_release.load_components()

    with open(OUT_FILE, 'w') as f:
        f.write(changelog.format())

class Release:
    def __init__(self, name):
        self.name = name

        self.released = 'Unreleased'
        self.security = []
        self.added = []
        self.fixed = []
        self.changed = []
        self.removed = []
        self.deprecated = []

        self.components = None

    def add(self, entry: os.DirEntry):
        with open(entry) as f:
            content = f.read().strip()

        if entry.name == 'released':
            self.released = content
            return

        entry_type = entry.name.split('-')[0]

        match entry_type:
            case 'security':
                store = self.security
            case 'added':
                store = self.added
            case 'fixed':
                store = self.fixed
            case 'changed':
                store = self.changed
            case 'removed':
                store = self.removed
            case 'deprecated':
                store = self.deprecated
            case _:
                print(f'WARNING: Unknown entry type "{entry_type}" in "{self.name}"', file=sys.stderr)
                return;

        with open(entry) as f:
            store.append(content)

    def load_components(self):
        self.components = Components(self.name)

    def sections(self):
        categories = []

        if self.security:
            categories.append(('Security', self.security))
        if self.added:
            categories.append(('Added', self.added))
        if self.fixed:
            categories.append(('Fixed', self.fixed))
        if self.changed:
            categories.append(('Changed', self.changed))
        if self.removed:
            categories.append(('Removed', self.removed))
        if self.deprecated:
            categories.append(('Deprecated', self.deprecated))

        return categories

    def format(self):
        heading =f'{self.name} ({self.released})'

        s = f'## {heading}\n'

        if self.components and self.components.format():
            s += "\nThis release contains the following new component versions:\n\n"
            s += self.components.format()

        for (section, items) in self.sections():
            s += f'\n### {section}\n\n'

            for item in sorted(items):
                s += textwrap.TextWrapper(width = 80, initial_indent='- ', subsequent_indent='  ').fill(item)
                s += '\n'

        return s

    def __lt__(self, other):
        (x_major, x_minor, x_patch) = self.name.split('.', maxsplit=2)
        (y_major, y_minor, y_patch) = other.name.split('.', maxsplit=2)

        if x_major != y_major:
            return x_major < y_major
        elif x_minor != y_minor:
            return x_minor < y_minor
        else:
            x_alpha = x_patch[0].isalpha()
            y_alpha = y_patch[0].isalpha()
            if x_alpha != y_alpha:
                return x_alpha
            else:
                return x_patch < y_patch

class Changelog:
    def __init__(self, path, heading):
        self.path = path
        self.heading = heading
        self.previous = ''
        self.releases = []

    def load(self):
        with os.scandir(BASE_DIR) as it:
            for entry in it:
                if entry.is_dir():
                    release_names.append(entry.name)
                elif entry.is_file() and entry.name == 'previous':
                    with open(entry.path) as f:
                        self.previous = f.read()

        for release_name in release_names:
            release = Release(release_name)
            self.add(release)
            with os.scandir(os.path.join(BASE_DIR, release_name)) as it:
                for entry in it:
                    if entry.is_file():
                        if entry.name == 'components.json':
                            release.load_components()
                        else:
                            release.add(entry)

    def add(self, release: Release):
        self.releases.append(release)
        self.releases.sort()

    def format(self):
        s = ''

        if self.heading:
            s += f'# {self.heading}\n'

        for release in reversed(self.releases):
            if s != '':
                s += '\n'
            s += release.format()

        if self.previous:
            s += '\n'
            s += self.previous


        return s

class Components:
    def __init__(self, release_name, previous_release = None):
        self.release_name = release_name

        if previous_release:
            # Get packages (crates) and their versions in current workspace
            metadata = subprocess.run(['cargo', 'metadata', '--format-version=1', '--no-deps'], capture_output=True, check=True)
            packages = json.loads(metadata.stdout)['packages']
            packages.sort(key = lambda x: x['manifest_path'])

            # Get data from previous release
            with open(os.path.join(BASE_DIR, previous_release, 'components.json')) as f:
                prev_packages = json.load(f)

            self.components = {}
            for package in packages:
                name = package['name']
                if name not in IGNORED_PACKAGES:
                    version = package['version']
                    if  name in prev_packages:
                        self.add(name, version, prev_packages[name]['version'] != version)
                    else:
                        self.add(name, version, True)
        else:
            with open(os.path.join(BASE_DIR, release_name, 'components.json')) as f:
                self.components = json.load(f)

    def add(self, name, version, changed):
        if changed:
            state = 'changed'
        else:
            state = 'unchanged'

        self.components[name] = { 'version': version, 'state': state }


    def write(self):
        with open(os.path.join(BASE_DIR, self.release_name, 'components.json'), 'w') as f:
            json.dump(self.components, f, indent=4)

    def format(self):
        s = ""
        for (component_name, component) in self.components.items():
            if component['state'] == "changed":
                s += f"- {component_name} {component['version']}\n"

        return s

main()
