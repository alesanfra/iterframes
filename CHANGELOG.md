# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
From 0.4.0 on, [release-please](https://github.com/googleapis/release-please)
writes each entry from the Conventional Commits since the previous release.

## [0.6.0](https://github.com/alesanfra/iterframes/compare/v0.5.0...v0.6.0) (2026-09-20)


### Features

* read approximate frames ([#11](https://github.com/alesanfra/iterframes/issues/11)) ([2158899](https://github.com/alesanfra/iterframes/commit/2158899f72f73d69c0020236181e340bb8648fc1))

## [0.5.0](https://github.com/alesanfra/iterframes/compare/v0.4.0...v0.5.0) (2026-09-20)


### ⚠ BREAKING CHANGES

* `read_all` is removed; use `list(iterframes.read(...))`.

### Features

* build wheels for Windows ([#4](https://github.com/alesanfra/iterframes/issues/4)) ([39702a5](https://github.com/alesanfra/iterframes/commit/39702a5b55333824956827c5cf23b50cf150fb22))
* read frames by number, in the style of decord ([#10](https://github.com/alesanfra/iterframes/issues/10)) ([6d0627e](https://github.com/alesanfra/iterframes/commit/6d0627e65d357667ba28f274c780c3a58da541cb))
* read frames in batches with read_batches ([#6](https://github.com/alesanfra/iterframes/issues/6)) ([136ae14](https://github.com/alesanfra/iterframes/commit/136ae14ad6cccfa3459f69518a610dd94957d4c1))

## [0.4.0](https://github.com/alesanfra/iterframes/compare/v0.3.0...v0.4.0) (2026-09-19)

The rewrite: static FFmpeg wheels, frames without copies, and hardware
decoding.

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
