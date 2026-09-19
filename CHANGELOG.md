# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - Unreleased

### Breaking
- Python 3.11 or later is required, for the buffer protocol in the
  stable ABI
- `read_batch` is replaced by `read_all`, which returns NumPy arrays in
  the order of the video. `read_batch` returned `Frame` objects in reverse
  order
- Frames are C-contiguous arrays instead of strided views on a buffer
  padded at the end of each row
- `FrameReader` yields `Frame` objects, which support the buffer
  protocol, instead of `(bytearray, height, width)` tuples

### Added
- The wheels bundle a static FFmpeg 9.0, so FFmpeg no longer needs to be
  installed. One abi3 wheel per platform covers every CPython from 3.11
  on: Linux x86_64 and aarch64 (manylinux_2_28), macOS arm64
- AV1 decoding, through dav1d
- Errors are raised instead of ending the iteration silently:
  `FileNotFoundError` and other `OSError`s when the file cannot be opened,
  `ValueError` when it is not a video, `RuntimeError` when decoding fails
- `read` accepts `os.PathLike` paths
- `FFMPEG_VERSION`, the version of the linked FFmpeg
- Documentation site on Read the Docs

### Changed
- FFmpeg decodes on several threads, and waiting for a frame releases the
  GIL, so other Python threads keep running
- Frames reach NumPy without a copy: the arrays share memory with the
  FFmpeg frames
- The decoder thread stops as soon as the iterator is dropped
- A change of resolution or pixel format within a stream no longer stops
  the decoding
- Development uses [uv](https://docs.astral.sh/uv/); `requirements-dev.txt`,
  the Dockerfile, and the build scripts are replaced by `uv.lock` and
  `scripts/build-ffmpeg.sh`
- Tests compare the frames with PyAV instead of decord
- CI lints Rust and Python, tests every wheel, and publishes to PyPI
  through trusted publishing
- Updated to PyO3 0.29, ffmpeg-next 9, and Rust edition 2024

## [0.2.0] - 2021-01-01

Last release before the rewrite.
