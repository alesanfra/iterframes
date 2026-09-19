#!/usr/bin/env bash
# Build a minimal, static, LGPL FFmpeg that the wheels link into the
# extension, so that they need no FFmpeg on the user's machine. dav1d is
# built too, because FFmpeg's own AV1 decoder needs a hardware decoder.
#
# Requires curl, make, a C compiler, and python3 (for meson and ninja,
# which dav1d builds with).
#
# Usage: scripts/build-ffmpeg.sh [PREFIX]    (default: build/ffmpeg)
#
# Then build with:
#   PKG_CONFIG_PATH=$PWD/build/ffmpeg/lib/pkgconfig \
#     maturin build --release --features static
#
# The script is idempotent: it does nothing when PREFIX already holds the
# same versions for the same platform. CI caches PREFIX keyed on the hash
# of this file, so bump a version here to rebuild.
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-9.0.2}"
DAV1D_VERSION="1.5.4"
NASM_VERSION="3.02"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="${1:-$ROOT/build/ffmpeg}"
mkdir -p "$PREFIX"
PREFIX="$(cd "$PREFIX" && pwd)"
JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
BUILD_ID="ffmpeg $FFMPEG_VERSION dav1d $DAV1D_VERSION $(uname -sm)"

if [[ "$(cat "$PREFIX/VERSION" 2>/dev/null)" == "$BUILD_ID" ]]; then
    echo "$BUILD_ID already built in $PREFIX"
    exit 0
fi
# Leftovers of another version or platform would be linked in too.
rm -rf "${PREFIX:?}"/*

if [[ "$(uname -s)" == "Darwin" ]]; then
    # Objects built for a newer macOS than the wheel's tag trigger linker
    # warnings and may call missing symbols; keep them in step.
    export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
fi

# Sources and build tools are only needed until the install.
SRC="$(mktemp -d)"
trap 'rm -rf "$SRC"' EXIT
cd "$SRC"

# FFmpeg's x86 assembly needs NASM, which manylinux images do not ship.
# Without it decoding is several times slower, so build it when missing.
case "$(uname -m)" in
    x86_64 | i?86)
        if ! command -v nasm >/dev/null; then
            curl -fsSL "https://www.nasm.us/pub/nasm/releasebuilds/$NASM_VERSION/nasm-$NASM_VERSION.tar.xz" | tar xJ
            (cd "nasm-$NASM_VERSION" && ./configure --prefix="$SRC/nasm" && make -j"$JOBS" && make install)
            export PATH="$SRC/nasm/bin:$PATH"
        fi
        ;;
esac

if ! command -v meson >/dev/null || ! command -v ninja >/dev/null; then
    python3 -m venv "$SRC/tools"
    "$SRC/tools/bin/pip" install --quiet meson ninja
    export PATH="$SRC/tools/bin:$PATH"
fi

curl -fsSL "https://downloads.videolan.org/pub/videolan/dav1d/$DAV1D_VERSION/dav1d-$DAV1D_VERSION.tar.xz" | tar xJ
(
    cd "dav1d-$DAV1D_VERSION"
    meson setup build \
        --prefix="$PREFIX" \
        --libdir=lib \
        --buildtype=release \
        --default-library=static \
        -Denable_tools=false \
        -Denable_tests=false
    ninja -C build install
)
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig"

curl -fsSL "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz" | tar xJ
cd "ffmpeg-$FFMPEG_VERSION"

# --disable-autodetect keeps system libraries (zlib, iconv, VideoToolbox,
# X11, ...) out, so the wheel links against libc alone. It also drops
# threads, which are enabled again explicitly. Only the libraries and
# components needed to demux, decode, and convert frames are built.
./configure \
    --prefix="$PREFIX" \
    --enable-static \
    --disable-shared \
    --enable-pic \
    --disable-autodetect \
    --enable-pthreads \
    --enable-libdav1d \
    --pkg-config-flags=--static \
    --disable-programs \
    --disable-doc \
    --disable-network \
    --disable-avdevice \
    --disable-avfilter \
    --disable-swresample \
    --disable-encoders \
    --disable-muxers \
    --disable-devices \
    --disable-filters \
    --disable-hwaccels
make -j"$JOBS"
make install

# Make the .pc files relocatable, so that PREFIX keeps working after it is
# moved or restored from the CI cache under another path.
for pc in "$PREFIX"/lib/pkgconfig/*.pc; do
    sed -e "s|$PREFIX|\${prefix}|g" \
        -e 's|^prefix=.*|prefix=${pcfiledir}/../..|' "$pc" > "$pc.tmp"
    mv "$pc.tmp" "$pc"
done

echo "$BUILD_ID" > "$PREFIX/VERSION"
echo "$BUILD_ID installed in $PREFIX"
