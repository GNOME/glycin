#!/bin/bash

# Publish crates to crates.io

function release_crate () {
    OUTPUT=$(cargo publish -p $1 2>&1)
    EXIT_CODE=$?
    
    if [[ $EXIT_CODE -ne 0 ]]; then
        if [[ "${OUTPUT}" == *"already exists"* ]]; then
            echo "Already released doing nothing: ${OUTPUT}"
        else
    	echo "Failed to release crate: ${OUTPUT}"
    	exit $EXIT_CODE
        fi
    else
        echo "Released crate: ${OUTPUT}"
    fi
}

release_crate glycin-utils
release_crate glycin
