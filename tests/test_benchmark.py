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


def test_decoding_overlaps_with_work(video_path):
    """Decoding must run while Python code holds the GIL.

    Upscaling makes each frame slow enough to decode to time reliably.
    With per-frame work that takes as long as decoding, running the two
    one after the other would double the time; overlapping them should
    barely change it.
    """
    size = {"height": 1080, "width": 1920}

    def hold_the_gil(seconds):
        end = time.perf_counter() + seconds
        while time.perf_counter() < end:
            pass

    def decode(work=0.0):
        start = time.perf_counter()
        count = 0
        for _ in iterframes.read(video_path, **size):
            hold_the_gil(work)
            count += 1
        return time.perf_counter() - start, count

    decoding, count = decode()
    total, _ = decode(work=decoding / count)
    print(f"\ndecoding {decoding:.3f}s, decoding and work {total:.3f}s")
    assert total < 1.5 * decoding
