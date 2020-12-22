from iterframes import read_video


def test_integration():
    assert 5 == len(list(read_video(5)))
