# iterframes

[![PyPI](https://img.shields.io/pypi/v/iterframes.svg)](https://pypi.org/project/iterframes/)
[![CI](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml/badge.svg)](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml)
[![Documentation](https://readthedocs.org/projects/iterframes/badge/?version=latest)](https://iterframes.readthedocs.io)

Iterate over the frames of a video as NumPy arrays. FFmpeg decodes them on
a background thread, written in Rust, while your code processes the
previous ones.

```python
import iterframes

for frame in iterframes.read("video.mp4"):
    ...  # frame is a (height, width, 3) uint8 array of RGB pixels

# Resize while decoding
for frame in iterframes.read("video.mp4", height=224, width=224):
    ...
```

## Installation

```console
pip install iterframes
```

The wheels bundle FFmpeg and work on any CPython from 3.10 on, on Linux
(x86_64, aarch64) and macOS (Apple silicon).

## Documentation

<https://iterframes.readthedocs.io>: [reference](docs/reference.md) and
[development guide](docs/development.md).

## License

iterframes is released under the [LGPL-3.0](LICENSE). The wheels include
[FFmpeg](https://ffmpeg.org/), built under the LGPL, and
[dav1d](https://code.videolan.org/videolan/dav1d), under the BSD 2-clause
license.
