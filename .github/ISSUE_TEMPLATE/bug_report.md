---
name: Bug Report
about: Create a report to help us improve
title: '[BUG] '
labels: bug
assignees: ''
---

## Bug Description

A clear and concise description of what the bug is.

## Code Example

```python
import iterframes

# Minimal code example that reproduces the issue
for frame in iterframes.read("video.mp4"):
    ...
```

## Error Message

```
Full error message and traceback if applicable
```

## Video

The container and codec of the video, as printed by
`ffprobe -hide_banner video.mp4`. Attach the file if you can share it.

## Environment

- **OS:** [e.g., macOS 15, Ubuntu 24.04]
- **Python version:** [e.g., 3.13.2]
- **iterframes version:** [`iterframes.__version__`]
- **FFmpeg version:** [`iterframes.FFMPEG_VERSION`]
- **Installation method:** [wheel from PyPI, from source]
