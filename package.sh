#!/bin/bash

set -e

# Build the project
cargo build --all --release

# Get target triple
TARGET=${1:-$(rustc -vV | sed -n 's|host: ||p')}

# Determine naming convention
case "$TARGET" in
    *windows*)
        SHARED_EXT="dll"
        STATIC_EXT="lib"
        LIB_PREFIX=""
        ;;
    *apple*)
        SHARED_EXT="dylib"
        STATIC_EXT="a"
        LIB_PREFIX="lib"
        ;;
    *)
        SHARED_EXT="so"
        STATIC_EXT="a"
        LIB_PREFIX="lib"
        ;;
esac

LIB_NAME="rs_dfu"
TARGET_DIR="target/release"

# Create distribution structure
rm -rf dist
mkdir -p dist/lib dist/include

# Copy libraries
SHARED_LIB="${TARGET_DIR}/${LIB_PREFIX}${LIB_NAME}.${SHARED_EXT}"
STATIC_LIB="${TARGET_DIR}/${LIB_PREFIX}${LIB_NAME}.${STATIC_EXT}"

if [ -f "$SHARED_LIB" ]; then
    cp "$SHARED_LIB" "dist/lib/"
    echo "Copied: $SHARED_LIB"
fi

if [ -f "$STATIC_LIB" ]; then
    cp "$STATIC_LIB" "dist/lib/"
    echo "Copied: $STATIC_LIB"
fi

# Find and copy headers
find target/cxxbridge/rs-dfu -name "*.h" -exec cp {} dist/include/ \;

# Create archive
ARCHIVE_NAME="${LIB_NAME}-${TARGET}.tar.gz"
tar -czf "$ARCHIVE_NAME" --strip-component 1 dist/

echo "Created: $ARCHIVE_NAME"
