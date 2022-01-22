import timeit

import numpy as np
import pytest

from iterframes import read


def test_same_behavior_as_decord(video_path):
    from decord import VideoReader

    frame = next(read(video_path))
    decord_frame = VideoReader(video_path).next().asnumpy()

    assert frame.shape == decord_frame.shape
    np.testing.assert_equal(frame, decord_frame)


@pytest.mark.parametrize("height, width", [(540, 960), (135, 240)])
def test_same_behavior_as_decord_with_resize(video_path, height, width):
    from decord import VideoReader

    frame = next(read(video_path, height=height, width=width))
    decord_frame = (
        VideoReader(video_path, width=width, height=height).next().asnumpy()
    )

    assert frame.shape == decord_frame.shape
    np.testing.assert_equal(frame, decord_frame)


def test_whole_video(video_path):
    from decord import VideoReader

    vr = VideoReader(video_path)
    for frame in read(video_path):
        frame_decord = vr.next().asnumpy()
        np.testing.assert_equal(frame, frame_decord)


def _test_benchmark(video_path):
    from decord import VideoReader

    def read_with_iterframes():
        return list(read(video_path))

    def read_with_decord():
        vr = VideoReader(video_path)
        frames = []
        while True:
            try:
                frame = vr.next()
                frames.append(frame)
            except StopIteration:
                break
        return frames

    decord_bench = timeit.timeit(read_with_decord, number=1)
    iterframes_bench = timeit.timeit(read_with_iterframes, number=1)

    assert iterframes_bench < decord_bench
