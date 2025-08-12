#!/bin/bash

set -e

# Get target triple
TARGET=${1:-$(rustc -vV | sed -n 's|host: ||p')}

# Determine naming convention
case "$TARGET" in
    *windows*)
        STATIC_EXT="lib"
        LIB_PREFIX=""
        ;;
    *apple*)
        STATIC_EXT="a"
        LIB_PREFIX="lib"
        ;;
    *)
        STATIC_EXT="a"
        LIB_PREFIX="lib"
        ;;
esac

LIB_NAME="rs_dfu"
TARGET_DIR=${2:-"target"}
PROFILE_DIR=${3:-"release"}

# Create distribution structure
rm -rf dist
mkdir -p dist/cmake dist/include dist/lib

# Copy libraries
STATIC_LIB="${TARGET_DIR}/${PROFILE_DIR}/${LIB_PREFIX}${LIB_NAME}.${STATIC_EXT}"
if [ -f "$STATIC_LIB" ]; then
    cp "$STATIC_LIB" "dist/lib/"
    echo "Copied: $STATIC_LIB"
fi

# Copy headers
HEADER_FILE="${TARGET_DIR}/cxxbridge/rs-dfu/src/lib.rs.h"
if [ -f "$HEADER_FILE" ]; then
  cp "$HEADER_FILE" "dist/include/$LIB_NAME.h"
  echo "Copied: $HEADER_FILE"
fi

# Copy CMake configuration
CMAKE_CONFIG="cmake/${LIB_NAME}-config.cmake"
if [ -f "$CMAKE_CONFIG" ]; then
  cp "$CMAKE_CONFIG" "dist/cmake/"
  echo "Copied: $CMAKE_CONFIG"
fi

# Create archive
ARCHIVE_NAME="${LIB_NAME}-${TARGET}.tar.gz"
tar -czf "$ARCHIVE_NAME" --strip-component 1 dist/

echo "Created: $ARCHIVE_NAME"
