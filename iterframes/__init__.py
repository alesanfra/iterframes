"""Iterate over the frames of a video as NumPy arrays.

The frames are decoded on a background thread that never takes the GIL,
so the next ones are decoded while your code processes the current one.
"""

import os
from typing import Iterator, List, Optional, Union

import numpy as np

from .iterframes import FFMPEG_VERSION, Frame, FrameReader, __version__

__all__ = [
    "FFMPEG_VERSION",
    "Frame",
    "FrameReader",
    "__version__",
    "read",
    "read_all",
]

PathLike = Union[str, "os.PathLike[str]"]


def read(
    path: PathLike,
    height: Optional[int] = None,
    width: Optional[int] = None,
    prefetch_frames: int = 1,
) -> Iterator[np.ndarray]:
    """Yield the frames of the video at ``path``, in order.

    Each frame is a ``(height, width, 3)`` array of ``uint8`` RGB pixels.
    ``height`` and ``width`` resize the frames; when only one is given, the
    other keeps the size of the video. While your code processes a frame, a
    background thread decodes up to ``prefetch_frames`` frames ahead of it,
    without taking the GIL.
    """
    # The arrays share memory with the frames, which they keep alive.
    for frame in FrameReader(path, height, width, prefetch_frames):
        yield np.asarray(frame)


def read_all(
    path: PathLike,
    height: Optional[int] = None,
    width: Optional[int] = None,
) -> List[np.ndarray]:
    """Return every frame of the video at ``path`` in a list.

    Takes the same arguments as :func:`read`. The whole video is kept in
    memory, so use :func:`read` for long videos.
    """
    return list(read(path, height, width, prefetch_frames=16))
