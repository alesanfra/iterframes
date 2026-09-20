from pathlib import Path

import av
import numpy as np
import pytest

DATA = Path(__file__).parent / "data"


@pytest.fixture
def video_path():
    """The test video: 901 frames of 480x270, H.264 in MP4."""
    return DATA / "video_480x270.mp4"


@pytest.fixture
def av1_video_path():
    """A shorter test video, in AV1, which dav1d decodes."""
    return DATA / "video_av1_480x270.mp4"


@pytest.fixture
def decode_with_pyav():
    """A factory for reference frames, decoded and converted by PyAV."""

    def decode(path, height=None, width=None):
        with av.open(str(path)) as container:
            return [
                frame.reformat(
                    width=width, height=height, format="rgb24"
                ).to_ndarray()
                for frame in container.decode(video=0)
            ]

    return decode


@pytest.fixture
def pyav_frames(video_path, decode_with_pyav):
    """The frames of the test video, decoded by PyAV."""
    return decode_with_pyav(video_path)


@pytest.fixture
def assert_close():
    """A comparison of frames from two FFmpeg builds.

    The YUV to RGB conversion rounds differently across FFmpeg versions,
    so pixels may be off by one or two.
    """

    def compare(actual, expected):
        assert actual.shape == expected.shape
        diff = np.abs(actual.astype(np.int16) - expected.astype(np.int16))
        assert diff.mean() < 1

    return compare
