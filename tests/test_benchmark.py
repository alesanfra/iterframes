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


def test_slice_from_the_first_frame_costs_no_more(video_path):
    """Frames counted from the first must not pay for the index.

    ``start=0`` with ``step=1`` reads the video straight through, so
    stopping at the last frame costs what reading it all costs, and
    stopping early costs a fraction of it.
    """

    def read(**arguments):
        def decode():
            for _ in iterframes.read(video_path, **arguments):
                pass

        return best_of(decode)

    whole, to_the_end, first_ten = (
        read(),
        read(stop=901),
        read(stop=10),
    )
    print(
        f"\nwhole {whole:.3f}s, stop=901 {to_the_end:.3f}s, "
        f"stop=10 {first_ten:.3f}s"
    )
    assert to_the_end < 1.25 * whole
    assert first_ten < 0.5 * whole


def test_slice_from_the_middle_seeks_once(video_path):
    """A slice with ``step=1`` must seek once, then decode in order.

    Decoding the second half after one seek takes about half of what the
    whole video takes, plus the pass that indexes the file. Seeking again
    for every frame, or decoding from the first frame, would cost more
    than reading it all.
    """

    def read(**arguments):
        def decode():
            for _ in iterframes.read(video_path, **arguments):
                pass

        return best_of(decode)

    whole, second_half = read(), read(start=450)
    print(f"\nwhole {whole:.3f}s, start=450 {second_half:.3f}s")
    assert second_half < 0.75 * whole


def test_approximate_frames_decode_less(video_path):
    """Approximate frames must cost one decoded frame each.

    Reading frames in the middle of their group of pictures decodes every
    frame from the key frame on, while the nearest key frames decode one
    frame each. Both pay for the pass that indexes the file.
    """
    with av.open(str(video_path)) as container:
        frames = list(container.decode(video=0))
        keys = [index for index, frame in enumerate(frames) if frame.key_frame]
    # Frames far enough into their group of pictures to cost several
    # decoded frames each, which approximate reading skips.
    wanted = [key + 20 for key in keys if key + 20 < len(frames)][:20]

    def read(**arguments):
        def decode():
            for _ in iterframes.read(video_path, frames=wanted, **arguments):
                pass

        return best_of(decode)

    exact, approximate = read(), read(approximate=True)
    print(f"\nexact {exact:.3f}s, approximate {approximate:.3f}s")
    assert approximate < exact
