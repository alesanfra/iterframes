# IterFrames - python video decoder

IterFrames is a simple Python video decoder implemented in Rust.

## Use

```python
import iterframes

for frame in iterframes.read("video.mp4"):
  # frame is a numpy array
  pass
```

## Develop

To develop IterFrames you need:
* working rust toolchain (e.g. https://rustup.rs)
* working python 3.6+ environment

### Install ffmpeg dependencies

On *nix systems, `clang`, `pkg-config` and FFmpeg libraries (including development headers) are required.

On macOS:
```bash
brew install pkg-config ffmpeg
```

On Debian-based systems:
```bash
apt install -y clang libavcodec-dev libavformat-dev libavutil-dev pkg-config
```

Other `libav*-dev` and `libsw*-dev` packages may be required if you enable the corresponding features, e.g., `libavdevice-dev` for the `device` feature.

### Install python dependencies
IterFrames is built with [maturin](https://github.com/PyO3/maturin)

```bash
pip install maturin pytest
maturin develop
pytest tests
```


