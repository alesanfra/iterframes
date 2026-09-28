# API

Everything lives in the top-level `iterframes` module.

## read

```python
read(path, height=None, width=None, prefetch_frames=1, device="cpu", on_device=False, frames=None, start=0, stop=None, step=1, approximate=None, interpolation=None) -> Iterator[numpy.ndarray]
```

Yields the frames of the video at `path`, in order, or with `frames`, or
`start`, `stop`, and `step`, only those frames, in the order asked for.
Each frame is a C-contiguous, writable `numpy.ndarray` of shape
`(height, width, 3)` and dtype `uint8`, holding RGB pixels.

| Argument | Description |
| --- | --- |
| `path` | Path of the video, as a `str` or `os.PathLike` |
| `height` | Height of the frames. Defaults to the height of the video |
| `width` | Width of the frames. Defaults to the width of the video |
| `prefetch_frames` | How many decoded frames may wait for your code. Defaults to 1 |
| `device` | Where to decode: `"cpu"`, `"auto"`, or a name from `DEVICES`. Defaults to `"cpu"`. See [Hardware decoding](guides.md#hardware-decoding) |
| `on_device` | With `device="cuda"`, yield `CudaFrame` objects left on the GPU instead of arrays. See [Frames on the GPU](guides.md#frames-on-the-gpu) |
| `frames` | The numbers of the frames to read, in the order to read them. See [Reading frames by number](guides.md#reading-frames-by-number) |
| `start`, `stop`, `step` | The frames to read, as a slice of the video |
| `approximate` | With `frames`, read the key frame nearest to each of them instead, when it is within this many frames. `True` is any distance. See [Approximate frames](guides.md#approximate-frames) |
| `interpolation` | How frames are resized: `"nearest"`, `"bilinear"`, `"bicubic"`, `"area"`, or `"lanczos"`. Defaults to bilinear on the CPU. See [Resizing](guides.md#resizing) |

Frames are resized with bilinear interpolation, unless `interpolation`
says otherwise. When only one of `height`
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

## read_batches

```python
read_batches(path, batch_size, height=None, width=None, prefetch_frames=1, device="cpu", drop_last=False, frames=None, start=0, stop=None, step=1, approximate=None, interpolation=None) -> Iterator[numpy.ndarray]
```

Yields the frames of the video in batches, in order, for models that take
several frames at once. Each batch is a C-contiguous, writable
`numpy.ndarray` of shape `(batch_size, height, width, 3)` and dtype
`uint8`. The frames are decoded straight into it, so unlike `numpy.stack`
over the frames of `read`, making a batch costs your code no copy.

| Argument | Description |
| --- | --- |
| `batch_size` | Frames per batch, at least 1 |
| `prefetch_frames` | How many decoded frames may wait for your code, rounded up to whole batches. Defaults to 1, that is one batch |
| `drop_last` | Drop the last batch when the video ends before it is full. By default it is yielded with the frames left over |

The other arguments are those of [`read`](#read); `on_device` is not
supported. Every frame of a batch has the size of the first frame of the
video, or `height` and `width`.

```python
for batch in iterframes.read_batches("video.mp4", 12, height=224, width=224):
    print(batch.shape)  # (12, 224, 224, 3), except maybe the last one
```

## Errors

Errors are raised by the first `next()` on the iterator, not by the call
to `read`, because decoding starts only then.

| Exception | When |
| --- | --- |
| `FileNotFoundError`, `PermissionError`, `OSError` | The file cannot be opened. The exception carries `errno` and `filename` |
| `ValueError` | The file is not a video FFmpeg can read, or has no video stream |
| `RuntimeError` | Decoding fails after the video has been opened, for instance on a codec that the build does not include |
| `IndexError` | `frames` asks for a frame the video does not have |

```python
try:
    frames = list(iterframes.read("missing.mp4"))
except FileNotFoundError as error:
    print(error.filename)  # missing.mp4
```

## FrameReader

```python
FrameReader(path, height=None, width=None, prefetch_frames=1, device="cpu", on_device=False, batch_size=None, drop_last=False, frames=None, start=0, stop=None, step=1, approximate=None, interpolation=None)
```

The iterator behind `read` and `read_batches`. It yields `Frame` objects,
`CudaFrame` objects with `on_device=True`, or `Batch` objects with
`batch_size`.

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
See [Frames without a copy](guides.md#frames-without-a-copy).

## Batch

Frames decoded into a single block of memory, which the buffer protocol
exposes as a writable, C-contiguous `(frames, height, width, 3)` block of
unsigned bytes. `read_batches` wraps it with `numpy.asarray`, which copies
nothing; its pixels stay alive as long as the batch or any array or view on
it.

## Constants

| Name | Value |
| --- | --- |
| `__version__` | Version of iterframes, such as `"0.4.0"` |
| `FFMPEG_VERSION` | Version of the FFmpeg that iterframes is linked to, such as `"9.0.2"` |
| `DEVICES` | Names accepted by `device` besides `"auto"`, such as `("cpu", "mps")` |
