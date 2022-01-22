[![PyPI version](https://badge.fury.io/py/iterframes.svg)](https://badge.fury.io/py/iterframes)

# IterFrames - python video decoder

IterFrames is a simple Python video decoder implemented in Rust.

## Use

```python
import iterframes

for frame in iterframes.read("video.mp4"):
    # frame is a numpy array with shape (height, width, 3)
    pass
```

## Develop

To develop IterFrames you need:

* working rust toolchain (e.g. https://rustup.rs)
* working python 3.6+ environment

### Install ffmpeg dependencies

On *nix systems, `clang`, `pkg-config` and FFmpeg libraries (including development headers) are required.

On macOS:

```shell
brew install pkg-config ffmpeg
```

On Debian-based systems:

```shell
apt install -y clang libavcodec-dev libavformat-dev libavutil-dev pkg-config
```

Other `libav*-dev` and `libsw*-dev` packages may be required if you enable the corresponding features,
e.g., `libavdevice-dev` for the `device` feature.

### Install python dependencies

IterFrames is built with [maturin](https://github.com/PyO3/maturin). To start developing Iterframes first install all dev dependencies in you virtual env (python 3.6+ required):

```shell
pip install -r requirements-dev.txt
```

Then you can build the python package with:

```
maturin develop
```

This command will compile iterframe and install it in the active virtualenv.

## Build

Currently only Linux an MacOS are supported.


### Linux
To build a `manylinux2010` wheel just run from project root folder:

```shell
bash scripts/build_manylinux.sh
```

### MacOS

Ensure you have installed the required dependencies, then run:

```shell
bash scripts/build_macos.sh
```
