#!/usr/bin/env bash
set -euo pipefail

FFMPEG_VERSION="${FFMPEG_VERSION:-8.1.1}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_ROOT="${PROJECT_ROOT}/.cache/ffmpeg-${FFMPEG_VERSION}"
SOURCE_ROOT="${PROJECT_ROOT}/.cache/ffmpeg-src-${FFMPEG_VERSION}"
ARCHIVE="${PROJECT_ROOT}/.cache/ffmpeg-${FFMPEG_VERSION}.tar.xz"
DOWNLOAD_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"

emit_env() {
    if [ -n "${GITHUB_ENV:-}" ]; then
        {
            echo "FFMPEG_PREFIX=${INSTALL_ROOT}"
            echo "PKG_CONFIG_PATH=${INSTALL_ROOT}/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
            echo "LD_LIBRARY_PATH=${INSTALL_ROOT}/lib:${LD_LIBRARY_PATH:-}"
        } >> "$GITHUB_ENV"
    fi
}

if [ -f "${INSTALL_ROOT}/lib/pkgconfig/libavcodec.pc" ]; then
    emit_env
    exit 0
fi

mkdir -p "${PROJECT_ROOT}/.cache"

if [ ! -f "$ARCHIVE" ]; then
    curl --fail --location "$DOWNLOAD_URL" --output "$ARCHIVE"
fi

rm -rf "$SOURCE_ROOT"
mkdir -p "$SOURCE_ROOT"
tar --extract --file "$ARCHIVE" --xz --strip-components=1 --directory "$SOURCE_ROOT"

pushd "$SOURCE_ROOT" >/dev/null
./configure \
    --prefix="$INSTALL_ROOT" \
    --disable-doc \
    --disable-debug \
    --disable-programs \
    --disable-static \
    --enable-shared \
    --disable-autodetect \
    --enable-libmp3lame \
    --enable-zlib \
    --disable-everything \
    --enable-protocol=file \
    --enable-demuxer=aac \
    --enable-demuxer=ape \
    --enable-demuxer=flac \
    --enable-demuxer=matroska \
    --enable-demuxer=mov \
    --enable-demuxer=mp3 \
    --enable-demuxer=ogg \
    --enable-demuxer=wav \
    --enable-muxer=flac \
    --enable-muxer=ipod \
    --enable-muxer=matroska \
    --enable-muxer=matroska_audio \
    --enable-muxer=mp3 \
    --enable-muxer=mp4 \
    --enable-muxer=ogg \
    --enable-muxer=wav \
    --enable-decoder=aac \
    --enable-decoder=aac_fixed \
    --enable-decoder=alac \
    --enable-decoder=ape \
    --enable-decoder=flac \
    --enable-decoder=mjpeg \
    --enable-decoder=mp3 \
    --enable-decoder=mp3float \
    --enable-decoder=opus \
    --enable-decoder=pcm_alaw \
    --enable-decoder=pcm_f32be \
    --enable-decoder=pcm_f32le \
    --enable-decoder=pcm_f64be \
    --enable-decoder=pcm_f64le \
    --enable-decoder=pcm_mulaw \
    --enable-decoder=pcm_s8 \
    --enable-decoder=pcm_s16be \
    --enable-decoder=pcm_s16le \
    --enable-decoder=pcm_s24be \
    --enable-decoder=pcm_s24le \
    --enable-decoder=pcm_s32be \
    --enable-decoder=pcm_s32le \
    --enable-decoder=pcm_u8 \
    --enable-decoder=png \
    --enable-decoder=vorbis \
    --enable-encoder=flac \
    --enable-encoder=libmp3lame \
    --enable-encoder=pcm_f32le \
    --enable-encoder=pcm_s16le \
    --enable-encoder=pcm_s24le \
    --enable-parser=aac \
    --enable-parser=flac \
    --enable-parser=mjpeg \
    --enable-parser=mpegaudio \
    --enable-parser=opus \
    --enable-parser=png \
    --enable-parser=vorbis \
    --enable-bsf=aac_adtstoasc \
    --disable-filters \
    --enable-filter=ebur128
make -j"$(nproc)"
make install
popd >/dev/null

emit_env
