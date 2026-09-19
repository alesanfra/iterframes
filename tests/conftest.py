from pathlib import Path

import av
import numpy as np
import pytest

DATA = Path(__file__).parent / "data"


@pytest.fixture
def video_path():
    return DATA / "video_480x270.mp4"


def decode_with_pyav(path, height=None, width=None):
    """Reference frames, decoded and converted by PyAV."""
    with av.open(str(path)) as container:
        return [
            frame.reformat(
                width=width, height=height, format="rgb24"
            ).to_ndarray()
            for frame in container.decode(video=0)
        ]


@pytest.fixture
def pyav_frames(video_path):
    return decode_with_pyav(video_path)


def assert_close(actual, expected):
    """Compare frames from two FFmpeg builds.

    The YUV to RGB conversion rounds differently across FFmpeg versions,
    so pixels may be off by one or two.
    """
    assert actual.shape == expected.shape
    diff = np.abs(actual.astype(np.int16) - expected.astype(np.int16))
    assert diff.mean() < 1
