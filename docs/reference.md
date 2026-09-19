# Reference

Everything lives in the top-level `iterframes` module.

## read

```python
read(path, height=None, width=None, prefetch_frames=1, device="cpu", on_device=False) -> Iterator[numpy.ndarray]
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
| `on_device` | With `device="cuda"`, yield `CudaFrame` objects left on the GPU instead of arrays. See [Frames on the GPU](#frames-on-the-gpu) |

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

## read_batches

```python
read_batches(path, batch_size, height=None, width=None, prefetch_frames=1, device="cpu", drop_last=False) -> Iterator[numpy.ndarray]
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
FrameReader(path, height=None, width=None, prefetch_frames=1, device="cpu", on_device=False, batch_size=None, drop_last=False)
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

## Batch

Frames decoded into a single block of memory, which the buffer protocol
exposes as a writable, C-contiguous `(frames, height, width, 3)` block of
unsigned bytes. `read_batches` wraps it with `numpy.asarray`, which copies
nothing; its pixels stay alive as long as the batch or any array or view on
it.

## Hardware decoding

`device` moves decoding to a hardware device, which leaves more CPU to
the code that processes the frames. The names are PyTorch's:

| Name | Platform | Device |
| --- | --- | --- |
| `"mps"` | macOS | VideoToolbox, the media engine of Apple silicon |
| `"cuda"` | Linux, Windows | NVDEC on an NVIDIA GPU, through the driver installed on the machine |

`iterframes.DEVICES` lists the names the installed wheel supports, `"cpu"`
included. Unlike in PyTorch, the device only decodes: the frames reach
your code as NumPy arrays in memory, unless you keep them on an NVIDIA GPU
with [`on_device`](#frames-on-the-gpu).

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

## Frames on the GPU

With `device="cuda"`, each frame goes from the GPU to memory to become a
NumPy array. A model on the same GPU would then send it back.
`on_device=True` skips both copies: `read` yields `CudaFrame` objects that
stay on the GPU, which PyTorch, CuPy, JAX, and other libraries take
through [DLPack](https://dmlc.github.io/dlpack/latest/) without a copy.

The price is the format. The frames are in NV12, the GPU decoder's
format, not RGB:

| Attribute | Value |
| --- | --- |
| `y` | Luma plane, of shape `(height, width)` |
| `uv` | Chroma plane at half the resolution, of shape `(height / 2, width / 2, 2)` rounded up, U then V |
| `height`, `width` | Size of the frame |
| `format` | `"nv12"`, or `"p010"`/`"p016"` for videos of more than 8 bits, whose samples are `uint16` |
| `device` | The GPU, such as `"cuda:0"` |

The planes are `uint8` (or `uint16`) views of the decoder's memory, with
a row stride larger than the width. Converting them to RGB is up to you,
for instance in PyTorch, with the same conversion as iterframes applies on
the CPU (BT.601, limited range):

```python
import torch
import iterframes

def nv12_to_rgb(frame):
    y = torch.from_dlpack(frame.y).float()
    uv = torch.from_dlpack(frame.uv).float()
    uv = uv.repeat_interleave(2, 0).repeat_interleave(2, 1)
    uv = uv[: y.shape[0], : y.shape[1]]
    y = (y - 16) * (255 / 219)
    u = (uv[..., 0] - 128) * (255 / 224)
    v = (uv[..., 1] - 128) * (255 / 224)
    r = y + 1.402 * v
    g = y - 0.344136 * u - 0.714136 * v
    b = y + 1.772 * u
    return torch.stack([r, g, b], -1).round().clamp(0, 255).to(torch.uint8)

frames = iterframes.read(
    "video.mp4", height=224, width=224, device="cuda", on_device=True
)
for frame in frames:
    rgb = nv12_to_rgb(frame)  # (224, 224, 3) uint8 tensor on the GPU
    model(rgb.permute(2, 0, 1)[None].float() / 255)
```

- A frame's memory stays valid for as long as the frame, a plane, or a
  tensor made from one lives. Keep only the frames you need: each holds
  GPU memory.
- The pixels are in place when `read` yields the frame, whatever CUDA
  stream reads them.
- Every frame must be decoded on the GPU. A codec it does not support,
  such as AV1, raises `RuntimeError` instead of falling back to the CPU.
- `on_device` needs `device="cuda"`: `"auto"` might pick the CPU, and
  VideoToolbox frames already live in memory that the CPU shares.

This has not run on an NVIDIA GPU yet; please report how it works for
you.

## Constants

| Name | Value |
| --- | --- |
| `__version__` | Version of iterframes, such as `"0.4.0"` |
| `FFMPEG_VERSION` | Version of the FFmpeg that iterframes is linked to, such as `"9.0.2"` |
| `DEVICES` | Names accepted by `device` besides `"auto"`, such as `("cpu", "mps")` |
