# iterframes

[![PyPI](https://img.shields.io/pypi/v/iterframes.svg)](https://pypi.org/project/iterframes/)
[![CI](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml/badge.svg)](https://github.com/alesanfra/iterframes/actions/workflows/ci.yaml)
[![Documentation](https://readthedocs.org/projects/iterframes/badge/?version=latest)](https://iterframes.readthedocs.io)

**Video frames as NumPy arrays, decoded while your code is busy with the
last one.**

```python
import iterframes

for frame in iterframes.read("video.mp4", height=224, width=224):
    model(frame)  # (224, 224, 3) uint8 RGB, resized by FFmpeg, no copy
```

`model` runs on one frame while a Rust thread decodes the next ones. That
thread never takes the GIL, so decoding overlaps with your work instead of
adding to it, even when your code is pure Python. `pip install iterframes`
brings FFmpeg with it: nothing else to install, no system packages, no
`ffmpeg` binary to call.

## Numbers

One video, 901 frames of 480x270 H.264, decoded and converted to RGB,
best of seven runs on an Apple silicon Mac with the bundled FFmpeg 9.0.2.
The benchmarks are in
[`tests/test_benchmark.py`](https://github.com/alesanfra/iterframes/blob/main/tests/test_benchmark.py);
run them on your own videos before believing them.

| | Time |
| --- | --- |
| Every frame, iterframes | **0.046 s** |
| Every frame, PyAV, same decode and conversion | 0.208 s |
| 32 frames at random positions | 0.036 s |
| First 10 frames (`stop=10`) | 0.003 s |
| Second half (`start=450`) | 0.028 s |

In the overlap benchmark, which upscales the same video to 1080p, adding
per-frame work as expensive as decoding took 16% longer in total, not
twice as long: the decoding had already happened.

## What you get

- **Frames without copies.** RGB `uint8` arrays of shape
  `(height, width, 3)`, straight out of FFmpeg's buffers.
- **Batches as one array.** `read_batches` decodes into a single
  `(batch, height, width, 3)` block, ready for a model.
- **Resizing while decoding**, not a `cv2.resize` afterwards.
- **Random access.** Read frames by number, without decoding the rest.
- **Hardware decoding** on NVIDIA GPUs (NVDEC) and Apple silicon
  (VideoToolbox), in the wheels. On NVIDIA the frames can stay on the GPU
  for PyTorch.
- **Wheels** for Linux (x86_64, aarch64), macOS (Apple silicon), and
  Windows, for every CPython from 3.11 on.

```python
# Batches of 12 frames, decoded into one (12, 224, 224, 3) array
for batch in iterframes.read_batches("video.mp4", 12, height=224, width=224):
    ...

# Stop whenever you like; the decoder stops with the loop
for index, frame in enumerate(iterframes.read("video.mp4")):
    if index == 100:
        break
```

## Random access

Sample clips for training, or grab a thumbnail, without reading the whole
file:

```python
clip = list(iterframes.read("video.mp4", frames=[0, 30, 60]))
every_fifth = iterframes.read("video.mp4", start=100, stop=200, step=5)
last = next(iterframes.read("video.mp4", frames=[-1]))
```

Both work with `read_batches`. Negative numbers count from the end, and
frame `n` is the one `read` yields `n`-th. iterframes indexes the file
once, without decoding it, then decodes each frame from the key frame
before it; frames asked for in order cost no more than reading the video
straight through. See
[Reading frames by number](https://iterframes.readthedocs.io/en/latest/reference/#reading-frames-by-number).

## Hardware decoding

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

The frames still arrive as NumPy arrays in memory. A GPU saves CPU time
but is not always faster than the CPU decoder, so measure both; see
[Hardware decoding](https://iterframes.readthedocs.io/en/latest/reference/#hardware-decoding).

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

Pick iterframes to read videos into a model, in order or by frame number,
and keep decoding out of your loop's way. Pick PyAV when you need the rest
of FFmpeg: audio, encoding, streams, or precise control over the decoder.
OpenCV is the natural choice when the rest of the pipeline already uses
it.

## Installation

```console
pip install iterframes
```

The wheels bundle FFmpeg and work on any CPython from 3.11 on, on Linux
(x86_64, aarch64), macOS (Apple silicon), and Windows (x86_64). Other
platforms build from source, FFmpeg included.

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
