import timeit
from pathlib import Path

from iterframes import read


def test_benchmark(video_path):
    video_path = str(video_path)
    from decord import VideoReader

    def read_with_iterframes():
        return list(read(video_path, prefetch_frames=5))

    def read_with_decord():
        vr = VideoReader(video_path)
        f = []
        while True:
            try:
                frame = vr.next()
                f.append(frame)
            except StopIteration:
                break
        return f

    iterframes_bench = timeit.timeit(read_with_iterframes, number=1)
    decord_bench = timeit.timeit(read_with_decord, number=1)

    print("Iterframes:\t", iterframes_bench)
    print("Decord:\t\t", decord_bench)


here = Path(__file__).parent

# test_benchmark(here.parent / "tests/data/1280x720.mp4")
test_benchmark(here.parent / "tests/data/video_480x270.mp4")
