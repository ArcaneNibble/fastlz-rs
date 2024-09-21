#!/bin/bash

set -xe

if [ ! -d FastLZ ]; then
    git clone https://github.com/ariya/FastLZ.git
fi

CLANG="${CLANG:-/opt/homebrew/opt/llvm/bin/clang}"
"$CLANG" \
    --target=wasm32 \
    -Os \
    -DFASTLZ_USE_MEMMOVE=0 \
    -nostdlib -fno-builtin \
    -Wl,--stack-first \
    -Wl,--no-entry \
    -Wl,--export=fastlz_compress \
    -Wl,--export=fastlz_compress_level \
    -Wl,--export=fastlz_decompress \
    -o fastlz.wasm \
    FastLZ/fastlz.c
