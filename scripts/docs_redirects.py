"""Redirect the old reference page, which became the API and Guides pages.

mkdocs-redirects keeps the anchor but not its page, and the sections of
the old page went to two pages, so each anchor is mapped here.
"""

import json
import os

OLD = "reference"
GUIDES = [
    "reading-frames-by-number",
    "approximate-frames",
    "threads",
    "hardware-decoding",
    "frames-on-the-gpu",
]

PAGE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Redirecting</title>
<noscript><meta http-equiv="refresh" content="0; url=../api/"></noscript>
<script>
var guides = {guides};
var anchor = location.hash.slice(1);
var page = guides.indexOf(anchor) >= 0 ? "../guides/" : "../api/";
location.replace(page + location.hash);
</script>
</head>
<body><a href="../api/">API</a> · <a href="../guides/">Guides</a></body>
</html>
"""


def on_post_build(config, **kwargs):
    path = os.path.join(config["site_dir"], OLD, "index.html")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as file:
        file.write(PAGE.format(guides=json.dumps(GUIDES)))
