#!/usr/bin/env python3
# A rather rudimentary fallback for 'cp' on envs that don't have it by default

import argparse
import shutil
import sys

def copy_file(argv):
    parser = argparse.ArgumentParser(
                        prog='cp-fallback',
                        description='Attempts to do what \'cp\' does when it is not available for our needs')
    parser.add_argument('sourcefile',
                        help='File to copy')
    parser.add_argument('destfile',
                        help='Destination file path')
    parser.add_argument('-a', '--archive',
                        help='Copy the attributes for the file as far as possible',
                        action='store_true')
    parsed_args = parser.parse_args()

    src = parsed_args.sourcefile
    dst = parsed_args.destfile

    if parsed_args.archive:
        shutil.copy2(src, dst)
    else:
        shutil.copy(src, dst)

if __name__ == '__main__':
    copy_file(sys.argv)
