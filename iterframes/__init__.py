"""Iterate over the frames of a video as NumPy arrays.

The frames are decoded on a background thread that never takes the GIL,
so the next ones are decoded while your code processes the current one.
"""

import os
from typing import Iterator, List, Optional, Union

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
    "read_all",
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
) -> Iterator[Union[np.ndarray, CudaFrame]]:
    """Yield the frames of the video at ``path``, in order.

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
    """
    reader = FrameReader(
        path, height, width, prefetch_frames, device, on_device
    )
    if on_device:
        yield from reader
        return
    # The arrays share memory with the frames, which they keep alive.
    for frame in reader:
        yield np.asarray(frame)


def read_all(
    path: PathLike,
    height: Optional[int] = None,
    width: Optional[int] = None,
    device: str = "cpu",
) -> List[np.ndarray]:
    """Return every frame of the video at ``path`` in a list.

    Takes the same arguments as :func:`read`. The whole video is kept in
    memory, so use :func:`read` for long videos.
    """
    return list(read(path, height, width, prefetch_frames=16, device=device))


def read_batches(
    path: PathLike,
    batch_size: int,
    height: Optional[int] = None,
    width: Optional[int] = None,
    prefetch_frames: int = 1,
    device: str = "cpu",
    drop_last: bool = False,
) -> Iterator[np.ndarray]:
    """Yield the frames of the video at ``path`` in batches, in order.

    Each batch is a ``(batch_size, height, width, 3)`` array of ``uint8``
    RGB pixels. The frames are decoded straight into it, so it costs no
    copy. The last batch holds the frames left over, unless ``drop_last``
    drops it. The frames of a batch have the size of the first frame of the
    video, or ``height`` and ``width``.

    Takes the same arguments as :func:`read`, except ``on_device``. The
    background thread decodes up to ``prefetch_frames`` frames ahead,
    rounded up to whole batches.
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
    )
    # The arrays share memory with the batches, which they keep alive.
    for batch in reader:
        yield np.asarray(batch)
