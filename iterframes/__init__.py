from typing import Optional
import numpy as np

from .iterframes import FrameReader as _FrameReader


def read(
    path: str,
    height: Optional[int] = None,
    width: Optional[int] = None,
    prefetch_frames: Optional[int] = None,
):
    fr = _FrameReader(path, height, width, prefetch_frames)

    while True:
        res = fr.next()
        if res is None:
            break
        else:
            frame, height, width, stride = res
            yield np.lib.stride_tricks.as_strided(
                frame, (height, width, 3), (stride, 3, 1)
            )
