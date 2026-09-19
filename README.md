# iterframes

[![PyPI](https://img.shields.io/pypi/v/iterframes.svg)](https://pypi.org/project/iterframes/)
[![CI](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml/badge.svg)](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml)
[![Documentation](https://readthedocs.org/projects/iterframes/badge/?version=latest)](https://iterframes.readthedocs.io)

A plain Python loop over the frames of a video, for when each frame goes
through something expensive, such as a model. While your code works on one
frame, FFmpeg decodes the next ones on a background thread, written in
Rust, that never takes the GIL. Decoding overlaps with your work instead
of adding to it.

```python
import iterframes

for frame in iterframes.read("video.mp4"):
    model(frame)  # frame is a (height, width, 3) uint8 array of RGB pixels

# Resize while decoding
for frame in iterframes.read("video.mp4", height=224, width=224):
    ...
```

### Hardware decoding

Pass `device` to decode on a GPU instead of the CPU, which then stays free
for your model. The names are PyTorch's:

```python
# NVIDIA GPU on Linux and Windows; with height and width, the GPU resizes too
for frame in iterframes.read("video.mp4", height=224, width=224, device="cuda"):
    ...

# Apple silicon (VideoToolbox)
for frame in iterframes.read("video.mp4", device="mps"):
    ...

# Whatever the machine has, else the CPU
for frame in iterframes.read("video.mp4", device="auto"):
    ...

print(iterframes.DEVICES)  # ('cpu', 'mps') on a Mac
```

By default the frames still arrive as NumPy arrays in memory. A GPU saves
CPU time but is not always faster than the CPU decoder, so measure both;
see [Hardware decoding](docs/reference.md#hardware-decoding).

With an NVIDIA GPU, `on_device=True` keeps the frames on it, in NV12, for
PyTorch and other libraries to take without a copy:

```python
import torch

for frame in iterframes.read("video.mp4", device="cuda", on_device=True):
    y = torch.from_dlpack(frame.y)    # (height, width) uint8, on the GPU
    uv = torch.from_dlpack(frame.uv)  # (height / 2, width / 2, 2)
```

[Frames on the GPU](docs/reference.md#frames-on-the-gpu) shows how to
convert them to RGB there.

## Installation

```console
pip install iterframes
```

The wheels bundle FFmpeg and work on any CPython from 3.11 on, on Linux
(x86_64, aarch64), macOS (Apple silicon), and Windows (x86_64).

## Documentation

<https://iterframes.readthedocs.io>: [reference](docs/reference.md) and
[development guide](docs/development.md).

## License

iterframes is released under the [LGPL-3.0](LICENSE). The wheels include
[FFmpeg](https://ffmpeg.org/), built under the LGPL, and
[dav1d](https://code.videolan.org/videolan/dav1d), under the BSD 2-clause
license.
