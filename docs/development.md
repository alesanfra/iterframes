# Development

You need [uv](https://docs.astral.sh/uv/), a Rust toolchain from
[rustup](https://rustup.rs/), and the tools to build FFmpeg: a C compiler,
`make`, `curl`, `python3`, `pkg-config`, and libclang. Only Linux and
macOS are supported.

```console
xcode-select --install && brew install pkg-config        # macOS
sudo apt install build-essential curl python3-venv \
    pkg-config libclang-dev nasm                          # Debian, Ubuntu
```

## FFmpeg

The extension always links the same static FFmpeg as the wheels, and
`build.rs` builds it: the first build downloads FFmpeg and dav1d and
compiles them into `build/ffmpeg`, which takes a few minutes. Later builds
reuse it, and so does anything else that runs `build.rs`, such as
`cargo clippy` or rust-analyzer. The log is in `build/ffmpeg.log`.

The versions and options live in `scripts/build-ffmpeg.sh`; after editing
it, the next build rebuilds FFmpeg. Set `ITERFRAMES_FFMPEG_DIR` to keep
the build somewhere else, for instance to share it between checkouts.

## Setup

```console
git clone https://github.com/alesanfra/iterframes.git
cd iterframes
uv venv -p 3.14
uv sync --frozen
uv run --no-sync pre-commit install
uv run maturin develop --uv
```

`maturin develop` compiles the Rust extension into the virtual
environment. Run it again after every change to a `.rs` file.

!!! warning "Use `--no-sync` after building"
    `uv run` re-syncs the environment by default, which replaces the module
    you just built. Run everything else as `uv run --no-sync ...`.

## Layout

| Path | Contents |
| --- | --- |
| `src/lib.rs` | Python module: `Frame`, `FrameReader`, and the error mapping |
| `src/decoder.rs` | Decoding thread: demux, decode, convert to RGB |
| `src/ffmpeg.rs` | Safe wrappers over the FFmpeg calls the crate needs |
| `iterframes/__init__.py` | `read` and `read_all`, which wrap `FrameReader` |
| `scripts/build-ffmpeg.sh` | Static FFmpeg build for the wheels |
| `tests/` | pytest suite, which checks the frames against PyAV |
| `docs/` | This site |

## Tests

```console
uv run --no-sync pytest
```

The tests compare the frames with those decoded by
[PyAV](https://github.com/PyAV-Org/PyAV). The conversion to RGB rounds
differently across FFmpeg versions, so pixels are compared with a small
tolerance.

A benchmark against PyAV is skipped by default. Build in release mode
first:

```console
uv run maturin develop --uv --release
uv run --no-sync pytest -m benchmark -s
```

## Linting

```console
uv run --no-sync pre-commit run -a
```

This runs `cargo fmt`, `cargo clippy`, `ruff format`, and `ruff check`. CI
runs the same checks.

## Documentation

```console
uv sync --frozen --group docs
uv run --no-sync mkdocs serve
```

Read the Docs builds the site with `uv sync` from `uv.lock`, without
compiling the extension.

## Wheels

The wheels use the stable ABI of CPython (abi3), so one wheel per platform
covers every CPython from 3.11 on. To build one locally:

```console
uv run --no-sync maturin build --release
```

On Linux, run the build in a
[manylinux](https://github.com/pypa/manylinux) container, as CI does, so
that the wheel works on older distributions.

## Pull requests

Add tests for new behavior, update the docs and `CHANGELOG.md`, and use
[Conventional Commits](https://www.conventionalcommits.org/) messages
(`feat: ...`, `fix: ...`).

## Releasing

1. Bump the version in `Cargo.toml`. `pyproject.toml` reads it from there.
2. Add a `CHANGELOG.md` entry.
3. Tag the commit with the bare version (`0.4.0`, no `v` prefix) and push
   the tag.

`.github/workflows/ci.yaml` builds the wheels, attests them, and publishes
them to PyPI through trusted publishing.
