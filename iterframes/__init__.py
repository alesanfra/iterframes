"""Iterate over the frames of a video as NumPy arrays."""

import os
from typing import Iterator, List, Optional, Union

import numpy as np

from .iterframes import FFMPEG_VERSION, FrameReader, __version__

__all__ = ["FFMPEG_VERSION", "FrameReader", "__version__", "read", "read_all"]

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
    other keeps the size of the video. A background thread decodes up to
    ``prefetch_frames`` frames ahead of the one being processed.
    """
    for buffer, frame_height, frame_width in FrameReader(
        path, height, width, prefetch_frames
    ):
        yield np.frombuffer(buffer, dtype=np.uint8).reshape(
            frame_height, frame_width, 3
        )


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
