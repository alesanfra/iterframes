"""Iterate over the frames of a video as NumPy arrays.

The frames are decoded on a background thread that never takes the GIL,
so the next ones are decoded while your code processes the current one.
"""

import os
from typing import Iterator, Optional, Sequence, Union

import numpy as np

from .iterframes import (
    DEVICES,
    FFMPEG_VERSION,
    Batch,
    CudaFrame,
    Frame,
    FrameReader,
    Plane,
    __version__,
)

__all__ = [
    "DEVICES",
    "Batch",
    "FFMPEG_VERSION",
    "CudaFrame",
    "Frame",
    "FrameReader",
    "Plane",
    "__version__",
    "read",
    "read_batches",
]

PathLike = Union[str, "os.PathLike[str]"]


def read(
    path: PathLike,
    height: Optional[int] = None,
    width: Optional[int] = None,
    prefetch_frames: int = 1,
    device: str = "cpu",
    on_device: bool = False,
    frames: Optional[Sequence[int]] = None,
    start: int = 0,
    stop: Optional[int] = None,
    step: int = 1,
    approximate: Union[bool, int, None] = None,
) -> Iterator[Union[np.ndarray, CudaFrame]]:
    """Yield the frames of the video at ``path``, in order.

    With ``frames``, or ``start``, ``stop``, and ``step``, only those
    frames are read, in the order asked for.

    Each frame is a ``(height, width, 3)`` array of ``uint8`` RGB pixels.
    ``height`` and ``width`` resize the frames; when only one is given, the
    other keeps the size of the video. While your code processes a frame, a
    background thread decodes up to ``prefetch_frames`` frames ahead of it,
    without taking the GIL.

    ``device`` is where to decode, named as in PyTorch: ``"cpu"``,
    ``"mps"`` (Apple's VideoToolbox), or ``"cuda"`` (NVIDIA's NVDEC); see
    :data:`DEVICES`. ``"auto"`` picks the first hardware device that opens
    and falls back to the CPU, while a hardware name raises
    ``RuntimeError`` when its device cannot be opened. Codecs the device
    does not support are decoded on the CPU either way. The frames reach
    your code as NumPy arrays in memory.

    With ``device="cuda"``, ``on_device=True`` leaves the frames on the GPU
    instead, as :class:`CudaFrame` objects in NV12 whose planes PyTorch and
    other libraries take through DLPack without a copy. The frames must
    then be decoded on the GPU: other codecs raise ``RuntimeError``.

    ``frames`` reads the frames with those numbers, in that order, instead
    of the whole video; ``start``, ``stop``, and ``step`` read a slice of
    it, with the meaning they have when slicing a list. Negative numbers
    count from the end of the video, and a number the video does not have
    raises ``IndexError``. ``start=0`` with ``step=1`` reads the video
    straight through and stops at ``stop``; every other selection indexes
    the file first, by reading its packets without decoding them, and then
    decodes each frame from the key frame before it, so frames asked for
    in order cost no more than reading the video straight through.

    ``approximate`` trades accuracy for speed, and goes with ``frames``
    only: each frame asked for is read as the key frame nearest to it, so
    it costs one decoded frame instead of the frames from the key frame
    on. ``approximate=n`` moves a frame by at most ``n`` frames and reads
    the rest exactly, which bounds the error; ``approximate=True`` moves
    it by any distance. The video still has to be indexed, so the packets
    are read either way. Two frames near the same key frame are then the
    same frame, and are decoded once each.
    """
    reader = FrameReader(
        path,
        height,
        width,
        prefetch_frames,
        device,
        on_device,
        frames=frames if frames is None else list(frames),
        start=start,
        stop=stop,
        step=step,
        approximate=approximate,
    )
    if on_device:
        yield from reader
        return
    # The arrays share memory with the frames, which they keep alive.
    for frame in reader:
        yield np.asarray(frame)


def read_batches(
    path: PathLike,
    batch_size: int,
    height: Optional[int] = None,
    width: Optional[int] = None,
    prefetch_frames: int = 1,
    device: str = "cpu",
    drop_last: bool = False,
    frames: Optional[Sequence[int]] = None,
    start: int = 0,
    stop: Optional[int] = None,
    step: int = 1,
    approximate: Union[bool, int, None] = None,
) -> Iterator[np.ndarray]:
    """Yield the frames of the video at ``path`` in batches, in order.

    Each batch is a ``(batch_size, height, width, 3)`` array of ``uint8``
    RGB pixels. The frames are decoded straight into it, so it costs no
    copy. The last batch holds the frames left over, unless ``drop_last``
    drops it. The frames of a batch have the size of the first frame of the
    video, or ``height`` and ``width``.

    Takes the same arguments as :func:`read`, except ``on_device``, so
    ``frames``, or ``start``, ``stop``, and ``step``, batch the frames
    with those numbers instead of the whole video, and ``approximate``
    batches the key frames nearest to ``frames``. The background thread
    decodes up to ``prefetch_frames`` frames ahead, rounded up to whole
    batches.
    """
    if batch_size < 1:
        raise ValueError("batch_size must be at least 1")
    reader = FrameReader(
        path,
        height,
        width,
        prefetch_frames,
        device,
        batch_size=batch_size,
        drop_last=drop_last,
        frames=frames if frames is None else list(frames),
        start=start,
        stop=stop,
        step=step,
        approximate=approximate,
    )
    # The arrays share memory with the batches, which they keep alive.
    for batch in reader:
        yield np.asarray(batch)
