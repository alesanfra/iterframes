# Reference

Everything lives in the top-level `iterframes` module.

## read

```python
read(path, height=None, width=None, prefetch_frames=1) -> Iterator[numpy.ndarray]
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

Frames are resized with bilinear interpolation. When only one of `height`
and `width` is given, the other keeps the size of the video, so the aspect
ratio changes.

Decoding starts when the first frame is requested, on a background thread
that runs ahead of your code by up to `prefetch_frames` frames. A larger
value smooths out frames that take longer to decode, at the cost of
memory: one 1080p frame takes about 6 MB.

The decoder stops when the iterator is exhausted or garbage-collected, for
example after a `break`.

```python
import iterframes

for frame in iterframes.read("video.mp4", height=270, width=480):
    print(frame.shape)  # (270, 480, 3)
```

## read_all

```python
read_all(path, height=None, width=None) -> list[numpy.ndarray]
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

The iterator behind `read`. It yields `(buffer, height, width)` tuples,
where `buffer` is a `bytearray` of `height * width * 3` bytes of packed
RGB pixels. Use it to skip NumPy, for instance to hand the bytes to
another library.

## Constants

| Name | Value |
| --- | --- |
| `__version__` | Version of iterframes, such as `"0.4.0"` |
| `FFMPEG_VERSION` | Version of the FFmpeg that iterframes is linked to, such as `"9.0.2"` |
