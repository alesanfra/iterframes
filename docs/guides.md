# Guides

What the arguments of [`read`](api.md#read) are for, and what they cost.

## Frames without a copy

Each frame is decoded into memory that iterframes allocates, and
swscale writes its RGB pixels there. `read` does not copy them into a new
NumPy array. The `Frame` that owns the memory implements Python's
[buffer protocol](https://docs.python.org/3/c-api/buffer.html): it tells
any library that asks where the pixels are, their shape
`(height, width, 3)`, their strides, and their type, `uint8`.
`numpy.asarray` builds the array on those same bytes.

What this changes for your code:

- **No copy per frame.** A copy would read and write every pixel once
  more, about 6 MB for a 1080p frame. Wrapping costs the same at any
  size.
- **Contiguous arrays.** The rows have no padding, so the array is
  C-contiguous. Libraries that need contiguous memory take it as it is:
  `torch.from_numpy(frame)` shares the memory too.
- **Batches in one block.** With `read_batches`, swscale writes each
  frame into its slice of one allocation. The batch arrives as one
  `(batch, height, width, 3)` array, with no `numpy.stack`.
- **Memory you own.** The decoder never writes to a frame again once it
  hands it over, so the array is writable and you can change it in place.
  The memory is freed when the last array or view on it goes away.

Other libraries can read a `Frame` without NumPy; see
[`Frame`](api.md#frame).

## Resizing

`height` and `width` resize each frame while swscale converts it to RGB,
so the full-size RGB frame is never made. `interpolation` picks the
method:

```python
frames = iterframes.read("video.mp4", height=224, width=224, interpolation="area")
```

| `interpolation` | Method | For |
| --- | --- | --- |
| `"nearest"` | The nearest pixel | Masks and labels, whose values must not blend. Fastest, but aliases |
| `"bilinear"` | Bilinear, the default | Most models |
| `"bicubic"` | Bicubic | Sharper edges than bilinear |
| `"area"` | Mean of the pixels each one covers | Shrinking |
| `"lanczos"` | Lanczos | The sharpest, and the slowest |

When they shrink a frame, `"bilinear"`, `"bicubic"`, and `"lanczos"`
widen their filter to cover every pixel of the source, as Pillow's
filters do, rather than blending only the nearest few. On a zone plate
shrunk from 1080p to 224x224, they land within 2.5 levels out of 255,
on average, of Pillow's filters of the same name.

On an Apple M2, a 1080p H.264 video of 300 frames and a 270p one of 901
frames, decoded and resized, best of five runs:

| `interpolation` | 1080p to 224x224 | 270p to 1080p |
| --- | --- | --- |
| None, no resizing | 0.171 s | 0.049 s |
| `"nearest"` | 0.146 s | 0.617 s |
| `"bilinear"` | 0.173 s | 0.786 s |
| `"bicubic"` | 0.202 s | 1.592 s |
| `"area"` | 0.163 s | 0.785 s |
| `"lanczos"` | 0.237 s | 2.152 s |

Shrinking costs little: converting a smaller frame to RGB saves about what
resizing it costs. Enlarging makes every frame that much bigger to
convert, and the method matters more.

With `device="cuda"` and no `interpolation`, the GPU resizes with
NVIDIA's own method, and only frames of the final size are copied to
memory. Naming an `interpolation` makes the CPU resize instead, from
frames copied at full size. It does not work with `on_device=True`.

## Reading frames by number

`frames` reads the frames with those numbers, in the order given, instead
of the whole video, and `start`, `stop`, and `step` read a slice of it,
with the meaning they have when slicing a list. Negative numbers count
from the end of the video. The two cannot be used together.

```python
clip = list(iterframes.read("video.mp4", frames=[0, 30, 60]))
every_fifth = iterframes.read("video.mp4", start=100, stop=200, step=5)
thumbnail = next(iterframes.read("video.mp4", frames=[-1]))
```

Both work with [`read_batches`](api.md#read_batches), which decodes the frames
straight into the batch:

```python
batch = next(iterframes.read_batches("video.mp4", 3, frames=[0, 30, 60]))
assert batch.shape[0] == 3
```

`start=0` with `step=1` needs no index: the video is read straight through
and stops at `stop`, which costs what reading it from the start costs.

Every other selection indexes the video first, by reading its packets
without decoding them, to find where each frame is and which frames
decoding can start from. Each frame asked for is then decoded from the key
frame before it, and the frames passed on the way are not converted to
RGB. Frames asked for in order cost no more than reading the video
straight through, since the decoder goes on from the frame it decoded last
whenever that is closer than the key frame.

Frame numbers are those of [`read`](api.md#read): frame `n` is the one `read`
yields `n`-th. A number the video does not have raises `IndexError`,
while a slice past the end stops at the last frame, as a list does.

## Approximate frames

Decoding a frame in the middle of a group of pictures costs every frame
from the key frame on. `approximate` spends one decoded frame instead, by
reading the key frame nearest to each frame asked for. It goes with
`frames` only; with `start`, `stop`, and `step` it raises `ValueError`.

```python
# The nearest key frame to each of these, however far away it is.
frames = list(iterframes.read("video.mp4", frames=[100, 200, 300], approximate=True))

# The nearest key frame within 5 frames, else the frame itself.
frames = list(iterframes.read("video.mp4", frames=[100, 200, 300], approximate=5))
```

A number bounds the error: a frame moves by at most that many frames, and
one with no key frame that close is decoded exactly. `True` accepts any
distance, which in a video with a key frame every 10 seconds means a frame
up to 5 seconds away from the one asked for. `False`, `0`, and `None`
read every frame exactly.

The video is still indexed, so its packets are read either way, and the
saving is in decoding alone. Two frames near the same key frame become
the same frame, which is decoded once for each of them: the iterator
yields as many frames as `frames` asks for, in the same order.

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
  interpolation differs slightly from the CPU's; see [Resizing](#resizing).

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
