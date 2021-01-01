import pytest

from iterframes import read




def test_integration(video_path):
    it = read(video_path)
    a = it.__next__()
    assert a.shape == (270, 480, 3)

