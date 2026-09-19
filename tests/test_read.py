import threading

import numpy as np
import pytest
from conftest import DATA, assert_close, decode_with_pyav

import iterframes


def test_version():
    assert iterframes.__version__
    assert iterframes.FFMPEG_VERSION


def test_frames_match_pyav(video_path, pyav_frames):
    frames = list(iterframes.read(video_path))

    assert len(frames) == len(pyav_frames) == 901
    for frame, expected in zip(frames, pyav_frames):
        assert_close(frame, expected)


def test_frame_layout(video_path):
    frame = next(iterframes.read(video_path))

    assert frame.shape == (270, 480, 3)
    assert frame.dtype == np.uint8
    assert frame.flags.c_contiguous
    assert frame.flags.writeable


def test_accepts_str_path(video_path):
    assert next(iterframes.read(str(video_path))).shape == (270, 480, 3)


@pytest.mark.parametrize("height, width", [(540, 960), (135, 240)])
def test_resize(video_path, height, width):
    expected = decode_with_pyav(video_path, height, width)

    frames = iterframes.read_all(video_path, height=height, width=width)

    assert len(frames) == len(expected)
    for frame, reference in zip(frames, expected):
        assert_close(frame, reference)


def test_resize_one_side_keeps_the_other(video_path):
    frame = next(iterframes.read(video_path, height=100))

    assert frame.shape == (100, 480, 3)


@pytest.mark.parametrize("prefetch_frames", [0, 1, 32])
def test_prefetch_frames(video_path, prefetch_frames):
    frames = iterframes.read(video_path, prefetch_frames=prefetch_frames)

    assert sum(1 for _ in frames) == 901


def test_read_all(video_path, pyav_frames):
    frames = iterframes.read_all(video_path)

    assert len(frames) == len(pyav_frames)
    assert_close(frames[-1], pyav_frames[-1])


def test_stop_early(video_path):
    # Dropping the reader must stop its decoder thread instead of leaving
    # it blocked on a full channel.
    for _ in range(20):
        for index, _ in enumerate(iterframes.read(video_path)):
            if index == 5:
                break


def test_threads_decode_in_parallel(video_path):
    results = []

    def count():
        results.append(sum(1 for _ in iterframes.read(video_path)))

    threads = [threading.Thread(target=count) for _ in range(4)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    assert results == [901] * 4


def test_missing_file(tmp_path):
    with pytest.raises(FileNotFoundError):
        next(iterframes.read(tmp_path / "missing.mp4"))


def test_not_a_video(tmp_path):
    path = tmp_path / "text.mp4"
    path.write_text("not a video")

    with pytest.raises(ValueError):
        next(iterframes.read(path))


def test_av1():
    # FFmpeg's native AV1 decoder only drives hardware decoders; the
    # software decoding comes from dav1d.
    path = DATA / "video_av1_480x270.mp4"
    expected = decode_with_pyav(path)

    frames = iterframes.read_all(path)

    assert len(frames) == len(expected) == 30
    for frame, reference in zip(frames, expected):
        assert_close(frame, reference)


def test_frames_share_memory_with_ffmpeg(video_path):
    frame = next(iterframes.FrameReader(video_path))
    array = np.asarray(frame)

    # Writing through the array changes the frame: no copy was made.
    array[0, 0] = (1, 2, 3)
    assert memoryview(frame).tobytes()[:3] == b"\x01\x02\x03"


def test_frame_buffer(video_path):
    # 470 * 3 bytes is not a multiple of 32, so swscale pads the rows.
    frame = next(iterframes.FrameReader(str(video_path), width=470))
    view = memoryview(frame)

    assert isinstance(frame, iterframes.Frame)
    assert view.shape == (270, 470, 3)
    assert view.format == "B"
    assert view.c_contiguous
    assert not view.readonly
    assert len(bytes(view)) == 270 * 470 * 3


def test_devices():
    assert iterframes.DEVICES[0] == "cpu"
    assert set(iterframes.DEVICES) <= {"cpu", "mps", "cuda"}


def test_unknown_device(video_path):
    with pytest.raises(ValueError, match="is unknown"):
        next(iterframes.read(video_path, device="nope"))


def test_device_auto_falls_back(video_path, pyav_frames):
    # Decodes on a device when there is one, on the CPU otherwise.
    frames = iterframes.read_all(video_path, device="auto")

    assert len(frames) == len(pyav_frames)
    for frame, expected in zip(frames, pyav_frames):
        assert_close(frame, expected)


@pytest.mark.parametrize("device", iterframes.DEVICES[1:])
@pytest.mark.parametrize("size", [None, (135, 240)])
def test_hardware_device(video_path, device, size):
    height, width = size or (None, None)
    expected = decode_with_pyav(video_path, height, width)
    try:
        frames = iterframes.read_all(video_path, height, width, device)
    except RuntimeError as error:
        pytest.skip(f"no {device} device: {error}")

    assert len(frames) == len(expected)
    for frame, reference in zip(frames, expected):
        assert_close(frame, reference)
