"""Reading frames by number instead of from start to end."""

from fractions import Fraction

import av
import numpy as np
import pytest

import iterframes


@pytest.fixture
def frames_of_the_video(video_path):
    """Every frame of the test video, read in order."""
    return list(iterframes.read(video_path))


@pytest.fixture
def encode_video(tmp_path):
    """A factory for short videos of flat frames, one color per frame.

    ``timestamps`` gives each frame a presentation time, in milliseconds,
    which makes the frame rate variable. ``skip`` drops that many packets
    from the start, which cuts the file in the middle of a group of
    pictures. The factory returns the path of the video it wrote.
    """

    def encode(name, timestamps=None, format=None, skip=0):
        path = tmp_path / name
        with av.open(str(path), "w", format=format) as container:
            stream = container.add_stream("libx264", rate=30)
            stream.width, stream.height = 64, 64
            stream.pix_fmt = "yuv420p"
            stream.codec_context.gop_size = 10
            if timestamps is not None:
                stream.codec_context.time_base = Fraction(1, 1000)
            packets = []
            for index in range(40):
                pixels = np.zeros((64, 64, 3), dtype=np.uint8)
                pixels[:, :, 0] = index * 6
                pixels[:, :, 1] = 255 - index * 6
                frame = av.VideoFrame.from_ndarray(pixels, format="rgb24")
                if timestamps is not None:
                    frame.pts = timestamps[index]
                    frame.time_base = Fraction(1, 1000)
                packets += stream.encode(frame)
            packets += stream.encode()
            for packet in packets[skip:]:
                container.mux(packet)
        return path

    return encode


@pytest.fixture
def variable_frame_rate(encode_video):
    """A video whose frames are not evenly spaced in time."""
    timestamps = np.cumsum([0] + [10 if i % 3 else 90 for i in range(39)])
    return encode_video(
        "variable.mp4", timestamps=[int(t) for t in timestamps]
    )


@pytest.fixture
def without_timestamps(encode_video):
    """A raw H.264 stream, which has neither timestamps nor an index."""
    return encode_video("raw.h264", format="h264")


@pytest.fixture
def cut_video(encode_video):
    """A video cut in the middle of a group of pictures."""
    return encode_video("cut.mp4", skip=3)


def test_frames_match_reading_in_order(video_path, frames_of_the_video):
    wanted = [0, 700, 12, 899, 700, 1]

    frames = list(iterframes.read(video_path, frames=wanted))

    assert len(frames) == len(wanted)
    for frame, index in zip(frames, wanted):
        np.testing.assert_array_equal(frame, frames_of_the_video[index])


def test_frames_match_pyav(video_path, decode_with_pyav, assert_close):
    expected = decode_with_pyav(video_path)

    frames = list(iterframes.read(video_path, frames=[5, 456, 900]))

    for frame, index in zip(frames, [5, 456, 900]):
        assert_close(frame, expected[index])


def test_negative_frames_count_from_the_end(video_path, frames_of_the_video):
    frames = list(iterframes.read(video_path, frames=[-1, -901]))

    np.testing.assert_array_equal(frames[0], frames_of_the_video[900])
    np.testing.assert_array_equal(frames[1], frames_of_the_video[0])


def test_no_frames(video_path):
    assert list(iterframes.read(video_path, frames=[])) == []


@pytest.mark.parametrize(
    "start, stop, step",
    [
        (0, 20, 1),
        (100, 200, 5),
        (450, None, 1),
        (890, None, 1),
        (0, None, 300),
        (-4, -1, 1),
    ],
)
def test_slice_matches_reading_in_order(
    video_path, frames_of_the_video, start, stop, step
):
    wanted = range(901)[start:stop:step]

    frames = list(
        iterframes.read(video_path, start=start, stop=stop, step=step)
    )

    assert len(frames) == len(wanted)
    for frame, index in zip(frames, wanted):
        np.testing.assert_array_equal(frame, frames_of_the_video[index])


def test_slice_past_the_end_stops_at_the_last_frame(video_path):
    frames = iterframes.read(video_path, start=895, stop=2000)

    assert sum(1 for _ in frames) == 6


def test_frame_out_of_range(video_path):
    with pytest.raises(IndexError, match="901 frames"):
        list(iterframes.read(video_path, frames=[901]))


def test_frames_and_slice_do_not_mix(video_path):
    with pytest.raises(ValueError, match="does not go with"):
        list(iterframes.read(video_path, frames=[1], start=2))


@pytest.mark.parametrize("step", [0, -1])
def test_step_must_be_positive(video_path, step):
    with pytest.raises(ValueError, match="step"):
        list(iterframes.read(video_path, step=step))


def test_frames_resize(video_path):
    frames = list(
        iterframes.read(video_path, height=135, width=240, frames=[300])
    )

    assert frames[0].shape == (135, 240, 3)


def test_batches_of_frames(video_path, frames_of_the_video):
    wanted = [0, 300, 600, 900, 450]

    batches = list(iterframes.read_batches(video_path, 2, frames=wanted))

    assert [len(batch) for batch in batches] == [2, 2, 1]
    stacked = np.concatenate(batches)
    for frame, index in zip(stacked, wanted):
        np.testing.assert_array_equal(frame, frames_of_the_video[index])


def test_batches_of_a_slice(video_path, frames_of_the_video):
    batches = list(
        iterframes.read_batches(video_path, 4, start=10, stop=30, step=2)
    )

    stacked = np.concatenate(batches)
    assert len(stacked) == 10
    np.testing.assert_array_equal(stacked[0], frames_of_the_video[10])
    np.testing.assert_array_equal(stacked[-1], frames_of_the_video[28])


def test_batches_of_frames_drop_last(video_path):
    batches = list(
        iterframes.read_batches(
            video_path, 4, frames=[0, 1, 2, 3, 4], drop_last=True
        )
    )

    assert [len(batch) for batch in batches] == [4]


@pytest.mark.parametrize(
    "video", ["variable_frame_rate", "without_timestamps", "cut_video"]
)
def test_odd_videos_match_reading_in_order(video, request):
    path = request.getfixturevalue(video)
    expected = list(iterframes.read(path))
    wanted = [0, 7, 25, 12, len(expected) - 1]

    frames = list(iterframes.read(path, frames=wanted))

    for frame, index in zip(frames, wanted):
        np.testing.assert_array_equal(frame, expected[index])


@pytest.mark.parametrize(
    "video", ["variable_frame_rate", "without_timestamps", "cut_video"]
)
def test_odd_videos_count_their_frames(video, request):
    path = request.getfixturevalue(video)

    # A slice past the end, so that the frames are picked by number
    # instead of read straight through.
    frames = iterframes.read(path, start=0, stop=1000)

    assert sum(1 for _ in frames) == len(list(iterframes.read(path)))


def test_frames_in_reverse_order(video_path, frames_of_the_video):
    wanted = list(range(900, 0, -150))

    frames = list(iterframes.read(video_path, frames=wanted))

    for frame, index in zip(frames, wanted):
        np.testing.assert_array_equal(frame, frames_of_the_video[index])


def test_slice_starting_past_the_end_is_empty(video_path):
    frames = iterframes.read(video_path, start=1000)

    assert sum(1 for _ in frames) == 0


@pytest.mark.parametrize("prefetch_frames", [0, 1, 32])
def test_frames_prefetch(video_path, prefetch_frames):
    frames = iterframes.read(
        video_path, prefetch_frames=prefetch_frames, frames=[0, 450, 900]
    )

    assert sum(1 for _ in frames) == 3


def test_frames_stop_early(video_path):
    # Dropping the reader must stop its decoder thread instead of leaving
    # it blocked on a full channel.
    for _ in range(20):
        for index, _ in enumerate(
            iterframes.read(video_path, start=0, stop=100)
        ):
            if index == 5:
                break


def test_frames_device_auto(video_path, frames_of_the_video, assert_close):
    frames = list(
        iterframes.read(video_path, device="auto", frames=[0, 300, 600])
    )

    for frame, index in zip(frames, [0, 300, 600]):
        assert_close(frame, frames_of_the_video[index])


def test_batches_of_frames_resize(video_path):
    batches = list(
        iterframes.read_batches(
            video_path, 2, height=135, width=470, frames=[0, 300, 600]
        )
    )

    assert [batch.shape for batch in batches] == [
        (2, 135, 470, 3),
        (1, 135, 470, 3),
    ]


def test_first_frames_match_reading_in_order(video_path, frames_of_the_video):
    frames = list(iterframes.read(video_path, stop=10))

    assert len(frames) == 10
    for frame, expected in zip(frames, frames_of_the_video):
        np.testing.assert_array_equal(frame, expected)


def test_stop_at_zero_reads_nothing(video_path):
    assert list(iterframes.read(video_path, stop=0)) == []


@pytest.mark.parametrize("stop", [901, 10**9])
def test_slice_to_the_end_reads_every_frame(video_path, stop):
    frames = iterframes.read(video_path, stop=stop)

    assert sum(1 for _ in frames) == 901
