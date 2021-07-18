from typing import Optional as _Optional

import numpy as _np

from .iterframes import FrameReader as _FrameReader, read_batch


def read(
    path: str,
    height: _Optional[int] = None,
    width: _Optional[int] = None,
    prefetch_frames: _Optional[int] = None,
):
    for res in _FrameReader(path, height, width, prefetch_frames):
        yield _np.lib.stride_tricks.as_strided(
            res.buffer, (res.height, res.width, 3), (res.stride, 3, 1)
        )
