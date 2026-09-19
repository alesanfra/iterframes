# Development

You need [uv](https://docs.astral.sh/uv/), a Rust toolchain from
[rustup](https://rustup.rs/), and the tools to build FFmpeg: a C compiler,
`make`, `curl`, `python3`, `pkg-config`, and libclang.

```console
xcode-select --install && brew install pkg-config        # macOS
sudo apt install build-essential curl python3-venv \
    pkg-config libclang-dev nasm                          # Debian, Ubuntu
```

On Windows, FFmpeg is built with MSVC from an
[MSYS2](https://www.msys2.org/) shell, as CI does in
`.github/workflows/ci.yaml`. Install Visual Studio's C++ build tools and
LLVM (for libclang), then, in MSYS2:

```console
pacman -S make diffutils curl tar xz \
    mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-nasm
rm /usr/bin/link.exe      # it shadows MSVC's link.exe
uv tool install meson && uv tool install ninja
```

Start the MSYS2 UCRT64 shell from a Visual Studio developer prompt with
`msys2_shell.cmd -ucrt64 -use-full-path`, so that `cl`, `cargo`, and `uv`
stay on `PATH`, and run every command below from it.

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
| `src/lib.rs` | Python module: `Frame`, `Batch`, `FrameReader`, and the error mapping |
| `src/decoder.rs` | Decoding thread: demux, decode, convert to RGB |
| `src/ffmpeg.rs` | Safe wrappers over the FFmpeg calls the crate needs |
| `iterframes/__init__.py` | `read`, `read_all`, and `read_batches`, which wrap `FrameReader` |
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

## Help wanted

Features that would fit iterframes but nobody has had time for yet. If
you want to work on one, open an issue first to agree on the API.

### Fast random access

Read frames at arbitrary positions, as decord does, for instance to
sample clips for training: `read(path, frames=[0, 30, 60])`, or `start`,
`stop`, and `step`. The usual approach:

1. When the video is opened, demux every packet without decoding it, which
   is cheap, to map each frame to its timestamp and find the keyframes.
2. For each requested frame, seek to the keyframe before it
   (`avformat_seek_file` with `AVSEEK_FLAG_BACKWARD`), then decode forward
   to it.
3. Skip the conversion to RGB of the frames decoded on the way, since it
   costs more than decoding them.
4. With sorted requests, decode forward instead of seeking again when the
   next frame is closer than the next keyframe.

The work belongs in `src/decoder.rs` and `src/ffmpeg.rs`, behind the same
background thread, so that it also works with `read_batches`. Videos with
a variable frame rate or broken timestamps need tests of their own.

### Testing on real hardware

CI has no GPU and no Windows 10 machine, so these have never run:

- `device="cuda"` and `on_device=True` on an NVIDIA GPU, on Linux and
  Windows. The tests skip when the device does not open; running
  `uv run --no-sync pytest` on a machine with a GPU and reporting the
  result helps.
- The Windows wheel on Windows 10.

## Pull requests

Add tests for new behavior and update the docs. Write the commit messages,
or the pull request title when squashing, as
[Conventional Commits](https://www.conventionalcommits.org/): they decide
the next version and become the changelog.

| Commit | Next version, before 1.0 | From 1.0 on |
| --- | --- | --- |
| `fix: ...`, `perf: ...` | patch | patch |
| `feat: ...` | minor | minor |
| `feat!: ...`, or a `BREAKING CHANGE:` footer | minor | major |
| `docs:`, `test:`, `ci:`, `chore:`, `refactor:` | none | none |

## Releasing

Releases are automated with
[release-please](https://github.com/googleapis/release-please), in
`.github/workflows/ci.yaml`:

1. Every push to `main` opens or updates a release pull request, which
   bumps the version in `Cargo.toml` and `Cargo.lock` and adds a
   `CHANGELOG.md` entry from the commits since the last release. Edit it
   freely before merging, for instance to add prose to the changelog; a
   later push to `main` regenerates it.
2. Merging it creates the tag, such as `v0.4.0`, and a draft GitHub
   release.
3. The same run builds and tests the wheels and the sdist, attaches them
   to the draft with their attestations, publishes them to PyPI through
   trusted publishing, and finally publishes the GitHub release.

Do not bump the version, edit released `CHANGELOG.md` entries, or push
tags by hand. `.release-please-manifest.json` holds the last released
version and `release-please-config.json` the settings.
