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


@pytest.mark.parametrize("batch_size", [1, 12, 901, 1000])
def test_batches_match_frames(video_path, batch_size):
    frames = list(iterframes.read(video_path))

    batches = list(iterframes.read_batches(video_path, batch_size))

    # 901 frames: every batch is full but the last.
    assert [len(batch) for batch in batches[:-1]] == [batch_size] * (
        len(batches) - 1
    )
    assert len(batches[-1]) == 901 - batch_size * (len(batches) - 1)
    np.testing.assert_array_equal(np.concatenate(batches), np.stack(frames))


def test_batch_layout(video_path):
    batch = next(iterframes.read_batches(video_path, 12))

    assert batch.shape == (12, 270, 480, 3)
    assert batch.dtype == np.uint8
    assert batch.flags.c_contiguous
    assert batch.flags.writeable


def test_batches_drop_last(video_path):
    batches = list(iterframes.read_batches(video_path, 12, drop_last=True))

    assert len(batches) == 901 // 12
    assert all(batch.shape[0] == 12 for batch in batches)


def test_batches_resize(video_path):
    # 470 * 3 bytes is not a multiple of 32: the rows must stay packed.
    frames = list(iterframes.read(video_path, height=135, width=470))

    batches = list(
        iterframes.read_batches(video_path, 12, height=135, width=470)
    )

    assert batches[0].shape == (12, 135, 470, 3)
    np.testing.assert_array_equal(np.concatenate(batches), np.stack(frames))


@pytest.mark.parametrize("prefetch_frames", [0, 1, 13, 100])
def test_batches_prefetch_frames(video_path, prefetch_frames):
    batches = iterframes.read_batches(
        video_path, 12, prefetch_frames=prefetch_frames
    )

    assert sum(len(batch) for batch in batches) == 901


def test_batches_stop_early(video_path):
    for _ in range(20):
        for index, _ in enumerate(iterframes.read_batches(video_path, 12)):
            if index == 2:
                break


def test_batches_device_auto(video_path):
    frames = list(iterframes.read(video_path, device="auto"))

    batches = list(iterframes.read_batches(video_path, 12, device="auto"))

    np.testing.assert_array_equal(np.concatenate(batches), np.stack(frames))


def test_batch_buffer(video_path):
    batch = next(iterframes.FrameReader(video_path, batch_size=3))
    view = memoryview(batch)

    assert isinstance(batch, iterframes.Batch)
    assert view.shape == (3, 270, 480, 3)
    assert view.format == "B"
    assert view.c_contiguous
    assert len(bytes(view)) == 3 * 270 * 480 * 3


@pytest.mark.parametrize("batch_size", [0, -1])
def test_batch_size_must_be_positive(video_path, batch_size):
    with pytest.raises(ValueError):
        next(iterframes.read_batches(video_path, batch_size))


def test_batches_not_on_device(video_path):
    with pytest.raises(ValueError, match="batch_size"):
        iterframes.FrameReader(
            video_path, device="cuda", on_device=True, batch_size=2
        )


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


def test_on_device_needs_cuda(video_path):
    with pytest.raises(ValueError, match="on_device=True needs"):
        next(iterframes.read(video_path, device="auto", on_device=True))


@pytest.mark.skipif("cuda" not in iterframes.DEVICES, reason="no NVDEC")
@pytest.mark.parametrize("size", [None, (135, 240)])
def test_on_device(video_path, size):
    height, width = size or (270, 480)
    try:
        frame = next(
            iterframes.read(
                video_path,
                *size or (None, None),
                device="cuda",
                on_device=True,
            )
        )
    except RuntimeError as error:
        pytest.skip(f"no cuda device: {error}")

    assert isinstance(frame, iterframes.CudaFrame)
    assert (frame.height, frame.width, frame.format) == (height, width, "nv12")
    assert frame.device.startswith("cuda:")
    assert frame.y.shape == [height, width]
    assert frame.uv.shape == [(height + 1) // 2, (width + 1) // 2, 2]
    assert frame.y.__dlpack_device__()[0] == 2  # kDLCUDA
    # An unconsumed capsule frees its tensor when collected.
    frame.y.__dlpack__()


@pytest.mark.skipif("cuda" not in iterframes.DEVICES, reason="no NVDEC")
def test_on_device_matches_cpu(video_path, pyav_frames):
    torch = pytest.importorskip("torch")
    try:
        frames = list(
            iterframes.read(video_path, device="cuda", on_device=True)
        )
    except RuntimeError as error:
        pytest.skip(f"no cuda device: {error}")

    assert len(frames) == len(pyav_frames)
    for frame, expected in zip(frames, pyav_frames):
        rgb = nv12_to_rgb(torch, frame).cpu().numpy()
        assert_close(rgb, expected)


def nv12_to_rgb(torch, frame):
    """The conversion shown in docs/reference.md."""
    y = torch.from_dlpack(frame.y).float()
    uv = torch.from_dlpack(frame.uv).float()
    uv = uv.repeat_interleave(2, 0).repeat_interleave(2, 1)
    uv = uv[: y.shape[0], : y.shape[1]]
    y = (y - 16) * (255 / 219)
    u = (uv[..., 0] - 128) * (255 / 224)
    v = (uv[..., 1] - 128) * (255 / 224)
    r = y + 1.402 * v
    g = y - 0.344136 * u - 0.714136 * v
    b = y + 1.772 * u
    return torch.stack([r, g, b], -1).round().clamp(0, 255).to(torch.uint8)
