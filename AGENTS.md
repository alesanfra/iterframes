# AGENTS.md

Working notes for coding agents (and humans) touching this repository.

## What this project is

`iterframes` is a Python extension module written in Rust with
[PyO3](https://pyo3.rs/) and built by [maturin](https://www.maturin.rs/).
It decodes videos with FFmpeg, through the raw bindings of `ffmpeg-sys-next`
wrapped in `src/ffmpeg.rs`, and yields
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

- Wheels link a static FFmpeg built by `scripts/build-ffmpeg.sh` into
  `build/ffmpeg`, via the crate feature `static` and `PKG_CONFIG_PATH`.
  The build disables autodetection, so the wheel depends on libc and
  system frameworks only; dav1d is added for AV1.
- Local development may link the system FFmpeg dynamically instead
  (default features).
- Cargo does not track the FFmpeg libraries: after changing them, run
  `cargo clean -p ffmpeg-sys-next`.
- Keep the build LGPL: never pass `--enable-gpl` or `--enable-nonfree`.
- Hardware decoding: VideoToolbox on macOS; on Linux the `*_cuvid`
  decoders, which load the NVIDIA driver with dlopen and resize on the
  GPU. Both add no library to the wheel. The static macOS build needs
  clang's compiler-rt for `@available`, which `build.rs` links.
- NVDEC has never run on a GPU in this project: CI has none, and the
  `cuda` tests skip when the device does not open.

## Layout

| Path | Contents |
| --- | --- |
| `src/lib.rs` | PyO3 module: `Frame`, `FrameReader`, error mapping, module init |
| `src/decoder.rs` | Decoding thread |
| `src/ffmpeg.rs` | Safe wrappers over the FFmpeg calls the crate needs |
| `iterframes/__init__.py` | `read`, `read_all` |
| `scripts/build-ffmpeg.sh` | Static FFmpeg and dav1d for the wheels |
| `tests/` | pytest suite; frames are compared with PyAV |
| `docs/` | MkDocs site published on Read the Docs |

## Environment

Requires [uv](https://docs.astral.sh/uv/), a Rust toolchain, `pkg-config`,
and FFmpeg (see `docs/development.md`).

```bash
uv venv -p 3.14                # once
uv sync --frozen               # dev dependencies
uv run maturin develop --uv    # compile the extension into the venv
```

With the static FFmpeg:

```bash
scripts/build-ffmpeg.sh
PKG_CONFIG_PATH=$PWD/build/ffmpeg/lib/pkgconfig uv run maturin develop --uv --features static
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

- Commits follow [Conventional Commits](https://www.conventionalcommits.org/).
- Rust: `cargo fmt` defaults, no `unwrap()` on anything reachable from
  Python input (a panic surfaces as `PanicException`).
- Python: ruff with a 79-column limit.
- Comments explain why, not what.

## Release

Version lives in `Cargo.toml` and is re-exported as
`iterframes.__version__`; `pyproject.toml` takes it from there. To
release: bump the version, update `CHANGELOG.md`, tag with the bare
version (`0.4.0`), and let `.github/workflows/ci.yaml` build and test the
wheels, attest them in the `release` job, and upload them in the `publish`
job. The upload uses PyPI trusted publishing, bound to this workflow file
and the `pypi` environment name: renaming either breaks publishing until
the publisher is updated on PyPI.
