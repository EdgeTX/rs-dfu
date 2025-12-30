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

# Create distribution structure
rm -rf dist
mkdir -p dist/cmake dist/include dist/lib

# Copy libraries
RELEASE_LIB="${TARGET_DIR}/release/${LIB_PREFIX}${LIB_NAME}.${STATIC_EXT}"
if [ -f "$RELEASE_LIB" ]; then
    cp "$RELEASE_LIB" "dist/lib/"
    echo "Copied: $RELEASE_LIB"
fi

DEBUG_LIB="${TARGET_DIR}/debug/${LIB_PREFIX}${LIB_NAME}.${STATIC_EXT}"
if [ -f "$DEBUG_LIB" ]; then
    cp "$DEBUG_LIB" "dist/lib/${LIB_PREFIX}${LIB_NAME}d.${STATIC_EXT}"
    echo "Copied: $DEBUG_LIB -> ${LIB_PREFIX}${LIB_NAME}d.${STATIC_EXT}"
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
