# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
From 0.4.0 on, [release-please](https://github.com/googleapis/release-please)
writes each entry from the Conventional Commits since the previous release.

## Unreleased

Notes written by hand for 0.4.0, the rewrite. release-please inserts its
0.4.0 entry below this section; fold the two together in the release pull
request.

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
- Hardware decoding with `device`, named as in PyTorch: `"mps"` for
  VideoToolbox on macOS, `"cuda"` for NVDEC on Linux, which also resizes
  on the GPU, or `"auto"`. `DEVICES` lists the devices of the build
- `on_device=True`, with `device="cuda"`, leaves the frames on the GPU as
  `CudaFrame` objects in NV12, whose planes support DLPack
- Documentation site on Read the Docs

### Changed
- FFmpeg decodes on several threads, and waiting for a frame releases the
  GIL, so other Python threads keep running
- The conversion to RGB, and the resizing, run on several threads too
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
- Updated to PyO3 0.29 and Rust edition 2024
- `ffmpeg-next` and `ffmpeg-sys-next` are replaced by `build.rs`, which
  builds the static FFmpeg, links it, and generates its bindings, so that
  `maturin build` needs no FFmpeg installed and no environment variables.
  The system FFmpeg is no longer used, and the `static` feature is gone

## [0.4.0](https://github.com/alesanfra/iterframes/compare/v0.3.0...v0.4.0) (2026-09-19)


### ⚠ BREAKING CHANGES

* `hwaccel` and `HWACCELS` are gone; use `device` and `DEVICES`.
* build FFmpeg from build.rs and drop ffmpeg-sys-next
* Python 3.11 or later is required, and FrameReader yields Frame objects instead of (bytearray, height, width) tuples.
* requires Python 3.10+; read_batch is replaced by read_all.

### Features

* decode on VideoToolbox or NVDEC with hwaccel ([e9c266a](https://github.com/alesanfra/iterframes/commit/e9c266a1441329306c2adfcba637226b1f1ff33d))
* hand frames to NumPy without a copy ([0d20709](https://github.com/alesanfra/iterframes/commit/0d2070980591af3b0323afbb389d8729b6a2535a))
* keep frames on the NVIDIA GPU with on_device ([e4f3637](https://github.com/alesanfra/iterframes/commit/e4f3637f538d6fcad64de77b443c0cb55eae1a5f))
* rename hwaccel to device, with PyTorch's names ([e2473f9](https://github.com/alesanfra/iterframes/commit/e2473f9ae75714aaefa2dbd705480e09fdd06f66))
* static FFmpeg abi3 wheels, uv, CI, and docs ([#2](https://github.com/alesanfra/iterframes/issues/2)) ([03cb2c3](https://github.com/alesanfra/iterframes/commit/03cb2c35423327f1ad336dfbf3712d25ad09aab2))


### Performance Improvements

* convert frames to RGB on several threads ([7fffaf6](https://github.com/alesanfra/iterframes/commit/7fffaf6dc10b56a4e64ece3b4ed6ccb57b9645c6))


### Build System

* build FFmpeg from build.rs and drop ffmpeg-sys-next ([80a73d3](https://github.com/alesanfra/iterframes/commit/80a73d3b389b3728b02d3e50ea720fdb6bb8285d))

## [0.3.0] - 2022-04-22

Tagged, but never published on PyPI.

### Added
- Resizing the frames while decoding

### Changed
- Frames are passed between threads through a crossbeam channel
- Updated FFmpeg and PyO3, Rust edition 2021, metadata in
  `pyproject.toml` (PEP 621)

### Fixed
- Resizing

## [0.2.0] - 2021-01-01

Last release before the rewrite.
