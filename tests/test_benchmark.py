import time

import av
import pytest

import iterframes

pytestmark = pytest.mark.benchmark


def best_of(function, repeat=5):
    timings = []
    for _ in range(repeat):
        start = time.perf_counter()
        function()
        timings.append(time.perf_counter() - start)
    return min(timings)


def test_decode_speed(video_path):
    def with_iterframes():
        for _ in iterframes.read(video_path, prefetch_frames=8):
            pass

    def with_pyav():
        with av.open(str(video_path)) as container:
            container.streams.video[0].thread_type = "AUTO"
            for frame in container.decode(video=0):
                frame.to_ndarray(format="rgb24")

    ours, theirs = best_of(with_iterframes), best_of(with_pyav)
    print(f"\niterframes {ours:.3f}s, PyAV {theirs:.3f}s")
