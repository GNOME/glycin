#!/bin/bash

# Publish crates to crates.io

function release_crate () {
    LOCAL_VERSION=$(cargo info "$1" | grep ^version:)
    LOCAL_VERSION=${LOCAL_VERSION#version: }
    LOCAL_VERSION=${LOCAL_VERSION% (*}
    PUBLISHED_VERSION=$(cd /tmp/; cargo info "$1@${LOCAL_VERSION}")

    if [ $? -ne 0 ]; then
        echo "Publishing '$1' with version '${LOCAL_VERSION}'."
        cargo publish -p $1 2>&1
    else
        echo "Crate '$1' with version '${LOCAL_VERSION}' already published. Skipping."
    fi
}

release_crate glycin-utils
release_crate glycin
