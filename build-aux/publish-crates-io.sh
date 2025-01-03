#!/bin/bash

# Publish crates to crates.io

function release_crate () {
    LOCAL_VERSION="$(cargo info "$1" | grep ^version:)"
    LOCAL_VERSION="${LOCAL_VERSION#version: }"
    LOCAL_VERSION="${LOCAL_VERSION% (*}"
    PUBLISHED_STATUS_CODE="$(curl -s -w "%{http_code}" -o /dev/null https://crates.io/api/v1/crates/glycin/2.1.0-alpha)"

    if [ "${PUBLISHED_STATUS_CODE}" -eq "404" ]; then
        echo "Publishing '$1' with version '${LOCAL_VERSION}'."
        cargo publish -p "$1"
    else
        echo "Crate '$1' with version '${LOCAL_VERSION}' already published (status code ${PUBLISHED_STATUS_CODE}). Skipping."
    fi
}

release_crate glycin-utils
release_crate glycin
