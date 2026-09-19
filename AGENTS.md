# AGENTS.md

Working notes for coding agents (and humans) touching this repository.

## What this project is

`iterframes` is a Python extension module written in Rust with
[PyO3](https://pyo3.rs/) and built by [maturin](https://www.maturin.rs/).
It decodes videos with a static FFmpeg, through bindings that `build.rs`
generates and `src/ffmpeg.rs` wraps, and yields
the frames as NumPy arrays of RGB pixels.

The point of the project is a plain Python loop over the frames in which
decoding overlaps with expensive work on each frame, such as model
inference: while the caller processes one frame, a background thread
decodes the next ones. Keep that true, and say so in the docs.

The package uses maturin's mixed layout: the compiled module is installed
as `iterframes.iterframes`, and `iterframes/__init__.py` wraps its
`FrameReader` into `read` and `read_all`.

## How decoding works

- `FrameReader::new` spawns a thread (`src/decoder.rs`) that demuxes,
  decodes, and converts each frame to RGB24 with swscale, and sends it
  through a bounded crossbeam channel of `prefetch_frames` slots.
- RGB frames are allocated with an alignment of 1, so their rows have no
  padding and Python can use them as contiguous arrays.
- Every call into FFmpeg, and so almost all the `unsafe` code, lives in
  `src/ffmpeg.rs`, behind small RAII types. Keep it that way: the rest of
  the crate stays safe.
- `__next__` waits on the channel with the GIL released (`py.detach`) and
  wraps the frame in a `Frame`, whose buffer protocol hands the pixels to
  NumPy without a copy. The buffer protocol needs `abi3-py311`.
- The decoder thread never takes the GIL, so it keeps decoding while
  Python code holds it. Never attach to Python there (no `Python::attach`,
  no Python objects in `decoder.rs`); `test_benchmark.py` checks the
  overlap.
- Errors travel through the channel and become Python exceptions in
  `impl From<Error> for PyErr`. A closed channel means the end of the video.
- Dropping the reader closes the channel; the thread notices on its next
  send and returns. Never `unwrap` a send.

## FFmpeg

- Every build, local or CI, links the static FFmpeg that `build.rs`
  builds with `scripts/build-ffmpeg.sh` into `build/ffmpeg` (or
  `$ITERFRAMES_FFMPEG_DIR`) the first time, under a file lock, logging to
  `build/ffmpeg.log`. The script is a no-op when `VERSION` in that
  directory matches what it would build. There is no system FFmpeg mode.
- `build.rs` links the libraries through `pkg-config --static` and runs
  bindgen on the headers, keeping only `av*_`/`sws_` items. Enums are
  newtype structs: use `.0` for the raw value. Function-like macros such
  as `AVERROR` are written by hand in the `sys` module of `src/ffmpeg.rs`.
- The build disables autodetection, so the wheel depends on libc and
  system frameworks only; dav1d is added for AV1.
- Keep the build LGPL: never pass `--enable-gpl` or `--enable-nonfree`.
- Hardware decoding (`device="mps"` / `"cuda"`, PyTorch's names, mapped
  to FFmpeg's in `hardware_devices` in `src/lib.rs`): VideoToolbox on
  macOS; on Linux the `*_cuvid`
  decoders, which load the NVIDIA driver with dlopen and resize on the
  GPU. Both add no library to the wheel. The macOS build needs clang's
  compiler-rt for `@available`, which `build.rs` links.
- NVDEC has never run on a GPU in this project: CI has none, and the
  `cuda` tests skip when the device does not open. The same goes for
  `on_device=True` (`CudaFrame`, `Plane`, `src/dlpack.rs`), which waits on
  cuvid's copy with CUDA driver calls found through dlopen
  (`ffmpeg::cuda`).

## Layout

| Path | Contents |
| --- | --- |
| `src/lib.rs` | PyO3 module: `Frame`, `FrameReader`, error mapping, module init |
| `src/decoder.rs` | Decoding thread |
| `src/ffmpeg.rs` | Safe wrappers over the FFmpeg calls the crate needs |
| `src/dlpack.rs` | DLPack capsules for the planes of `CudaFrame` |
| `iterframes/__init__.py` | `read`, `read_all` |
| `build.rs` | Builds and links FFmpeg, generates its bindings |
| `scripts/build-ffmpeg.sh` | Static FFmpeg and dav1d, run by `build.rs` |
| `tests/` | pytest suite; frames are compared with PyAV |
| `docs/` | MkDocs site published on Read the Docs |

## Environment

Requires [uv](https://docs.astral.sh/uv/), a Rust toolchain, and what
FFmpeg needs to build: a C compiler, `make`, `curl`, `python3`,
`pkg-config`, libclang (see `docs/development.md`).

```bash
uv venv -p 3.14                # once
uv sync --frozen               # dev dependencies
uv run maturin develop --uv    # build FFmpeg (first time) and the extension
```

**`uv run` re-syncs the project by default and overwrites the module that
`maturin develop` just built.** Always run tests and scripts as:

```bash
uv run --no-sync pytest
```

## Checks to run before proposing a change

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
uv run maturin develop --uv
uv run --no-sync pytest
```

CI runs the same four, plus `ruff check .` and `ruff format --check .` for
the Python files. `pre-commit run -a` covers the formatters and linters
locally.

## Docs

Three pages: `index.md` (overview), `reference.md` (API and errors),
`development.md`. Every example must match the behavior of a freshly
built module; check them instead of writing them from memory. Keep the
text short, in American English, with no performance claims that have not
been measured.

## Conventions

- Commits follow [Conventional Commits](https://www.conventionalcommits.org/),
  which release-please turns into versions and changelog entries.
- Rust: `cargo fmt` defaults, no `unwrap()` on anything reachable from
  Python input (a panic surfaces as `PanicException`).
- Python: ruff with a 79-column limit.
- Comments explain why, not what.

## Release

Version lives in `Cargo.toml` and is re-exported as
`iterframes.__version__`; `pyproject.toml` takes it from there. Never bump
it, write `CHANGELOG.md` entries, or push tags by hand: release-please
does, from the Conventional Commits, in the `release-please` job of
`.github/workflows/ci.yaml` (see "Releasing" in `docs/development.md`).
Tags are `vX.Y.Z`; before 1.0 a breaking change bumps the minor version.

Merging the release pull request creates the tag and a draft GitHub
release; the same run attaches the wheels (`release` job), uploads them
to PyPI (`publish`), and publishes the release (`publish-release`). The
jobs share one workflow because a tag pushed with the default
`GITHUB_TOKEN` starts no other workflow. The upload uses PyPI trusted
publishing, bound to this workflow file and the `pypi` environment name:
renaming either breaks publishing until the publisher is updated on
PyPI.
