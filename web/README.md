# Landing page

The page published at <https://alesanfra.github.io/iterframes/>. Plain HTML,
CSS and JavaScript: no build step, no framework, no web fonts. Open
`index.html` in a browser, or serve the directory, and what you see is what
gets deployed.

`.github/workflows/pages.yaml` deploys this directory, and only runs when
something inside it changes. `ci.yaml` ignores it for the same reason.

## Files

| Path | Contents |
| --- | --- |
| `index.html` | The whole page |
| `styles.css` | Every rule; the colour meanings are fixed at the top |
| `main.js` | The two animated diagrams and the copy buttons |
| `assets/frames.webp` | Sprite sheet, 12 frames of 160x90 in a row |
| `assets/scenes.webp` | Sprite sheet, frames 934, 4522 and 11711 at 240x135 |
| `assets/og.png` | Social card, 1200x630 |
| `assets/favicon.svg` | Favicon |

Animations are built from numbers declared in `main.js` (`FRAMES`, `DECODE`,
`WORK`, `TOTAL_FRAMES`, `GOP`, `WANTED`), and the captions are computed from
the same numbers. Change a number and the drawing and the text move together.

The hero plays both timelines on one clock, so the loop that decodes inline is
still running when the one built on iterframes has finished.

Colours mean the same thing in every diagram: yellow is frames and data, cyan
is the decoder thread, magenta is the GPU.

## Regenerating the images

The frames come from
[Big Buck Bunny](https://peach.blender.org/) at 640x360 (&copy; Blender
Foundation, [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/)), which
is also where the frame numbers on the page come from: the file has 14,316
frames at 24 fps, with a key frame every 90.

```bash
curl -O https://download.blender.org/peach/bigbuckbunny_movies/BigBuckBunny_640x360.m4v.zip
unzip BigBuckBunny_640x360.m4v.zip
src=BigBuckBunny_640x360.m4v

# assets/frames.webp: 12 consecutive frames, every second frame from 4:23
ffmpeg -ss 263 -i "$src" -frames:v 24 \
  -vf "select='not(mod(n\,2))',scale=160:90:flags=lanczos,tile=12x1" \
  -frames:v 1 -c:v libwebp -quality 74 -compression_level 6 assets/frames.webp

# assets/scenes.webp: the three frames the random access demo asks for
ffmpeg -i "$src" \
  -vf "select='eq(n\,934)+eq(n\,4522)+eq(n\,11711)',scale=240:135:flags=lanczos,tile=3x1" \
  -frames:v 1 -c:v libwebp -quality 76 -compression_level 6 assets/scenes.webp
```

The sprite sheets are read with `background-position` in percentages, so the
tiles must stay the same size and in the same order. A sheet of *n* tiles is
addressed as `calc(var(--i) * 100% / (n - 1))`.

`assets/og.png` is composed with `ffmpeg` too; the command is in the commit
that added it.

## Analytics

There is none, and no third-party script of any kind. GitHub offers nothing
for Pages either: the repository's Insights &rarr; Traffic panel counts visits
to the repository, not to this page.

To add a cookieless counter, replace the comment in the `<head>` of
`index.html`. [GoatCounter](https://www.goatcounter.com/) is hosted, free for
reasonable use, stores nothing on the visitor's machine and needs no consent
banner:

```html
<script data-goatcounter="https://NAME.goatcounter.com/count"
        async src="//gc.zgo.at/count.js"></script>
```

[Cloudflare Web Analytics](https://developers.cloudflare.com/web-analytics/)
is the other free cookieless option and works on `github.io` without moving
the domain, but ad blockers stop its beacon more often.
