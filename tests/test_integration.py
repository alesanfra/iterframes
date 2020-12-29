from iterframes import read


def test_integration(data_path):
    path = str(data_path / "video_480x270.mp4")
    print(path)
    it = read(path)

    a = it.__next__()

    assert a.shape == (3, 480, 270)
