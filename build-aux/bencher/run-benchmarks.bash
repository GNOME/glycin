#!/bin/bash

# Only run until significance level is reached to be fast enough
# for bencher.dev's 5 min limit. This makes it about 5x faster.
COMMON_ARGS="--quick --significance-level 0.005 --noplot --bench"

./bench-loader $COMMON_ARGS
./bench-thumbnailer $COMMON_ARGS
./bench-utils $COMMON_ARGS
