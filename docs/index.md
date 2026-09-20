# iterframes

iterframes is a plain Python loop over the frames of a video, for when
each frame goes through something expensive, such as a model:

```python
import iterframes

for frame in iterframes.read("video.mp4"):
    model(frame)  # frame is a (height, width, 3) uint8 array of RGB pixels
```

While `model` runs on one frame, FFmpeg decodes the next ones on a
background thread, written in Rust with [PyO3](https://pyo3.rs/). The
thread never takes the GIL, so it keeps decoding even while your code
holds it, and decoding overlaps with your work instead of adding to it.
The frames reach NumPy without a copy.

## Installation

```console
pip install iterframes
```

The wheels include FFmpeg, so nothing else needs to be installed. There is
one wheel per platform, which works on every CPython from 3.11 on:

| Platform | Architectures |
| --- | --- |
| Linux (glibc 2.28 or later) | x86_64, aarch64 |
| macOS 11 or later | arm64 (Apple silicon) |
| Windows 10 or later | x86_64 |

On other platforms pip builds from source, FFmpeg included, which takes a
few minutes and requires a Rust toolchain and a C toolchain; see
[Development](development.md).

## Quick start

Resize the frames while decoding:

```python
for frame in iterframes.read("video.mp4", height=224, width=224):
    assert frame.shape == (224, 224, 3)
```

Load a short clip into memory at once:

```python
frames = list(iterframes.read("clip.mp4"))
```

Feed a model 12 frames at a time, decoded into one array:

```python
for batch in iterframes.read_batches("video.mp4", 12, height=224, width=224):
    assert batch.shape[1:] == (224, 224, 3)
```

Read frames at given positions, without decoding the whole video:

```python
clip = list(iterframes.read("video.mp4", frames=[0, 30, 60]))
every_fifth = iterframes.read("video.mp4", start=100, stop=200, step=5)
```

Or, when a frame nearby will do, read the nearest key frame to each of
them and skip the decoding in between:

```python
clip = list(iterframes.read("video.mp4", frames=[0, 30, 60], approximate=5))
```

Stop whenever you like; the decoder stops with the loop:

```python
for index, frame in enumerate(iterframes.read("video.mp4")):
    if index == 100:
        break
```

The [reference](reference.md) describes every argument and the errors
raised.

## Formats

The bundled FFmpeg reads the containers and codecs FFmpeg supports on its
own, such as MP4, MKV, WebM, AVI, and MPEG-TS with H.264, H.265, VP8, VP9,
MPEG-4, ProRes, and MJPEG. AV1 is decoded by
[dav1d](https://code.videolan.org/videolan/dav1d). Only local files are
read: network protocols are left out of the build. Decoding runs on the
CPU unless you ask for a hardware device; see
[Hardware decoding](reference.md#hardware-decoding).
