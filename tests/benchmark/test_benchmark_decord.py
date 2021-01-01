import timeit

import numpy as np

from iterframes import read


def test_same_behavior_as_decord(video_path):
    from decord import VideoReader

    frame = read(video_path).__next__()
    decord_frame = VideoReader(video_path).next().asnumpy()

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
    vr = VideoReader(video_path)

    def read_with_iterframes():
        for frame in read(video_path, prefetch_frames=1):
            pass

    def read_with_decord():
        while True:
            try:
                frame = vr.next()
            except StopIteration:
                break

    decord_bench = timeit.timeit(read_with_decord, number=1)
    iterframes_bench = timeit.timeit(read_with_iterframes, number=1)

    assert iterframes_bench < decord_bench
