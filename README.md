# iterframes

[![PyPI](https://img.shields.io/pypi/v/iterframes.svg)](https://pypi.org/project/iterframes/)
[![CI](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml/badge.svg)](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml)
[![Documentation](https://readthedocs.org/projects/iterframes/badge/?version=latest)](https://iterframes.readthedocs.io)

Video frame reader for Python: iterate over the frames of a video as
NumPy arrays, decoded by FFmpeg on a background thread while your code
processes them.

iterframes is for pipelines that read a video from start to end and send
each frame, or each batch of frames, through something expensive: model
inference with PyTorch, ONNX Runtime, or TensorFlow, computer vision,
feature extraction, or labeling datasets. While your code works on one
frame, a background thread written in Rust decodes the next ones. The
thread never takes the GIL, so decoding overlaps with your work instead of
adding to it, even when your code is pure Python.

- `pip install iterframes`, with FFmpeg bundled: nothing else to install.
- RGB `uint8` arrays of shape `(height, width, 3)`, or batches of shape
  `(batch, height, width, 3)`, handed to NumPy without a copy.
- Resizing while decoding, with no separate resize step.
- Random access: read frames by number, without decoding the rest.
- Hardware decoding on NVIDIA GPUs (NVDEC) and Apple silicon
  (VideoToolbox); on NVIDIA, frames can stay on the GPU for PyTorch.
- Wheels for Linux (x86_64, aarch64), macOS (Apple silicon), and Windows,
  for every CPython from 3.11 on.

```python
import iterframes

for frame in iterframes.read("video.mp4"):
    model(frame)  # frame is a (height, width, 3) uint8 array of RGB pixels

# Resize while decoding
for frame in iterframes.read("video.mp4", height=224, width=224):
    ...

# Batches of 12 frames, decoded into one (12, 224, 224, 3) array
for batch in iterframes.read_batches("video.mp4", 12, height=224, width=224):
    ...

# Frames at given positions, without decoding the whole video
clip = list(iterframes.read("video.mp4", frames=[0, 30, 60]))
every_fifth = iterframes.read("video.mp4", start=100, stop=200, step=5)
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
see [Hardware decoding](https://iterframes.readthedocs.io/en/latest/reference/#hardware-decoding).

With an NVIDIA GPU, `on_device=True` keeps the frames on it, in NV12, for
PyTorch and other libraries to take without a copy:

```python
import torch

for frame in iterframes.read("video.mp4", device="cuda", on_device=True):
    y = torch.from_dlpack(frame.y)    # (height, width) uint8, on the GPU
    uv = torch.from_dlpack(frame.uv)  # (height / 2, width / 2, 2)
```

[Frames on the GPU](https://iterframes.readthedocs.io/en/latest/reference/#frames-on-the-gpu) shows how to
convert them to RGB there.

## Compared with OpenCV, decord, and PyAV

| | iterframes | OpenCV `VideoCapture` | decord | PyAV |
| --- | --- | --- | --- | --- |
| Decoding runs | Ahead of your code, on a background thread | When you call `read()` | When you index the reader | When you ask for the next frame |
| Pixels | RGB | BGR | RGB | Any format FFmpeg supports |
| Resize while decoding | Yes | No, with `cv2.resize` after | Yes | Yes, with `reformat` |
| Batches as one array | Yes, `read_batches` | No | Yes, `get_batch` | No |
| Seeking and random access | Yes, `frames=[...]` or `start`, `stop`, `step` | Yes | Yes, fast | Yes |
| Audio, encoding, muxing | No | Encoding with `VideoWriter` | Audio reading | Yes |
| Hardware decoding | NVIDIA, Apple silicon, in the wheels | Depends on the build and backend | NVIDIA, when built from source | Depends on the build |
| Latest wheels | Linux, macOS, Windows, CPython 3.11+ | Linux, macOS, Windows | x86_64 only, last release in 2021 | Linux, macOS, Windows |

Pick iterframes when you read videos sequentially, or sample frames from
them, and want decoding to stay out of the way of your model. Pick PyAV
when you need the rest of FFmpeg: audio, encoding, streams, or precise
control over the decoder. OpenCV is the natural choice when the rest of the
pipeline already uses it.

## Installation

```console
pip install iterframes
```

The wheels bundle FFmpeg and work on any CPython from 3.11 on, on Linux
(x86_64, aarch64), macOS (Apple silicon), and Windows (x86_64).

## Documentation

<https://iterframes.readthedocs.io>: [reference](https://iterframes.readthedocs.io/en/latest/reference/) and
[development guide](https://iterframes.readthedocs.io/en/latest/development/).

## Contributing

Contributions are welcome. The
[development guide](https://iterframes.readthedocs.io/en/latest/development/#help-wanted)
lists features that are waiting for someone to build them.

## License

iterframes is released under the [LGPL-3.0](https://github.com/alesanfra/iterframes/blob/main/LICENSE). The wheels include
[FFmpeg](https://ffmpeg.org/), built under the LGPL, and
[dav1d](https://code.videolan.org/videolan/dav1d), under the BSD 2-clause
license.
