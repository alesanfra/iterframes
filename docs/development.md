# Development

You need [uv](https://docs.astral.sh/uv/), a Rust toolchain from
[rustup](https://rustup.rs/), `pkg-config`, and FFmpeg. Only Linux and
macOS are supported.

## FFmpeg

The Rust bindings generate their code from the FFmpeg headers with
libclang, and work with any recent FFmpeg (7.1 and 9.0 are tested). There
are two ways to provide it.

### System FFmpeg

The quickest option for day-to-day work. The extension links dynamically
against the FFmpeg of your system:

```console
brew install ffmpeg pkg-config                    # macOS
sudo apt install libavcodec-dev libavformat-dev \
    libavutil-dev libswscale-dev libclang-dev pkg-config   # Debian, Ubuntu
```

### Static FFmpeg, as in the wheels

`scripts/build-ffmpeg.sh` downloads FFmpeg and dav1d and builds them as
static libraries in `build/ffmpeg`. This is what the published wheels
link, so use it to reproduce a wheel or a bug that only shows up there.
It takes a few minutes and needs `curl`, `make`, a C compiler, and
`python3`:

```console
scripts/build-ffmpeg.sh
export PKG_CONFIG_PATH=$PWD/build/ffmpeg/lib/pkgconfig
```

Then add `--features static` to the `maturin` commands below. The script
does nothing when `build/ffmpeg` is up to date; to change FFmpeg's version
or options, edit the script.

!!! note "Switching FFmpeg"
    Cargo does not notice when the FFmpeg libraries change. After
    switching between the system and the static FFmpeg, or rebuilding the
    latter, run `cargo clean -p ffmpeg-sys-next`.

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
covers every CPython from 3.11 on. To build one locally, with the static
FFmpeg from above:

```console
uv run --no-sync maturin build --release --features static
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
