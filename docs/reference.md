# Reference

Everything lives in the top-level `iterframes` module.

## read

```python
read(path, height=None, width=None, prefetch_frames=1, device="cpu") -> Iterator[numpy.ndarray]
```

Yields the frames of the video at `path`, in order. Each frame is a
C-contiguous, writable `numpy.ndarray` of shape `(height, width, 3)` and
dtype `uint8`, holding RGB pixels.

| Argument | Description |
| --- | --- |
| `path` | Path of the video, as a `str` or `os.PathLike` |
| `height` | Height of the frames. Defaults to the height of the video |
| `width` | Width of the frames. Defaults to the width of the video |
| `prefetch_frames` | How many decoded frames may wait for your code. Defaults to 1 |
| `device` | Where to decode: `"cpu"`, `"auto"`, or a name from `DEVICES`. Defaults to `"cpu"`. See [Hardware decoding](#hardware-decoding) |

Frames are resized with bilinear interpolation. When only one of `height`
and `width` is given, the other keeps the size of the video, so the aspect
ratio changes.

Decoding starts when the first frame is requested, on a background thread
that runs ahead of your code by up to `prefetch_frames` frames. The thread
never takes the GIL, so it decodes the next frames while your code
processes the current one, even when that code holds the GIL. A larger
`prefetch_frames` smooths out frames that take longer to decode, at the
cost of memory: one 1080p frame takes about 6 MB.

The decoder stops when the iterator is exhausted or garbage-collected, for
example after a `break`.

```python
import iterframes

for frame in iterframes.read("video.mp4", height=270, width=480):
    print(frame.shape)  # (270, 480, 3)
```

## read_all

```python
read_all(path, height=None, width=None, device="cpu") -> list[numpy.ndarray]
```

Returns every frame of the video in a list. It takes the same arguments as
[`read`](#read) and keeps the whole video in memory, so use `read` for
anything but short clips.

## Errors

Errors are raised by the first `next()` on the iterator, not by the call
to `read`, because decoding starts only then.

| Exception | When |
| --- | --- |
| `FileNotFoundError`, `PermissionError`, `OSError` | The file cannot be opened. The exception carries `errno` and `filename` |
| `ValueError` | The file is not a video FFmpeg can read, or has no video stream |
| `RuntimeError` | Decoding fails after the video has been opened, for instance on a codec that the build does not include |

```python
try:
    frames = iterframes.read_all("missing.mp4")
except FileNotFoundError as error:
    print(error.filename)  # missing.mp4
```

## Threads

Waiting for the next frame releases the GIL, and each call to `read` has
its own decoder, so several threads can read videos in parallel:

```python
from concurrent.futures import ThreadPoolExecutor

def count_frames(path):
    return sum(1 for _ in iterframes.read(path))

with ThreadPoolExecutor() as pool:
    counts = list(pool.map(count_frames, paths))
```

FFmpeg also spreads the decoding of each video over several threads.

## FrameReader

```python
FrameReader(path, height=None, width=None, prefetch_frames=1)
```

The iterator behind `read`. It yields `Frame` objects.

## Frame

A decoded frame. It holds the pixels and exposes them through the buffer
protocol as a writable, C-contiguous `(height, width, 3)` block of
unsigned bytes, so that other libraries can read them without a copy:

```python
import numpy as np
from iterframes import FrameReader

for frame in FrameReader("video.mp4"):
    array = np.asarray(frame)   # what read() yields
    view = memoryview(frame)    # no NumPy needed
```

The pixels stay alive as long as the frame or any array or view on it.

## Hardware decoding

`device` moves decoding to a hardware device, which leaves more CPU to
the code that processes the frames. The names are PyTorch's:

| Name | Platform | Device |
| --- | --- | --- |
| `"mps"` | macOS | VideoToolbox, the media engine of Apple silicon |
| `"cuda"` | Linux | NVDEC on an NVIDIA GPU, through the driver installed on the machine |

`iterframes.DEVICES` lists the names the installed wheel supports, `"cpu"`
included. Unlike in PyTorch, the device only decodes: the frames always
reach your code as NumPy arrays in memory.

```python
for frame in iterframes.read("video.mp4", device="auto"):
    ...
```

- `"auto"` uses the first device that opens and falls back to the CPU
  when there is none.
- A name raises `RuntimeError` on the first `next()` when its device
  cannot be opened, for instance without an NVIDIA driver.
- Codecs that the device does not support are decoded on the CPU either
  way. AV1 always is, by dav1d.
- With `"cuda"`, the GPU also does the resizing to `height` and `width`,
  so that only frames of the final size are copied to memory. Its
  interpolation differs slightly from the CPU's.

A device is not always faster. On Apple silicon, VideoToolbox decodes one
frame at a time: in our tests on 1080p H.264 and 4K HEVC it used 40% to
70% of the CPU time of the CPU decoder, but delivered 4 to 6 times fewer
frames per second. Measure both on your
videos and machine. NVDEC support has not been measured yet.

## Constants

| Name | Value |
| --- | --- |
| `__version__` | Version of iterframes, such as `"0.4.0"` |
| `FFMPEG_VERSION` | Version of the FFmpeg that iterframes is linked to, such as `"9.0.2"` |
| `DEVICES` | Names accepted by `device` besides `"auto"`, such as `("cpu", "mps")` |
