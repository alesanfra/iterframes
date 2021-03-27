from pathlib import Path

import pytest


@pytest.fixture
def data_path():
    return Path(__file__).parent / "data"


@pytest.fixture
def video_path(data_path):
    #return str(data_path / "video_480x270.mp4")
    return str(data_path / "ocean.mov")
