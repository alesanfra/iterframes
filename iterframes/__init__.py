from typing import Optional
import numpy as np
from .iterframes import read as _internal_read


def read(
    path: str,
    height: Optional[int] = None,
    width: Optional[int] = None,
    prefetch_frames: Optional[int] = None,
):
    for frame in _internal_read(path, height, width, prefetch_frames):
        if width is not None:
            frame = frame[:, np.arange(width)]
        yield frame
