"""Check that every local reference in the landing page resolves to a file.

The page has no build step, so nothing else would catch a renamed asset or a
typo in a path until the deployed site showed a broken image.
"""

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlparse

# href="...", src="..." and url(...) in CSS.
REFERENCES = re.compile(
    r"""(?:href|src)\s*=\s*["']([^"']+)["']|url\(\s*["']?([^"')]+)["']?\s*\)"""
)


def local(reference: str) -> bool:
    parsed = urlparse(reference)
    return not parsed.scheme and not parsed.netloc and bool(parsed.path)


def main(root: Path) -> int:
    missing = []
    for source in sorted(root.rglob("*")):
        if source.suffix not in {".html", ".css", ".js"}:
            continue
        text = source.read_text(encoding="utf-8")
        for match in REFERENCES.finditer(text):
            reference = match.group(1) or match.group(2)
            if not local(reference):
                continue
            path = unquote(urlparse(reference).path)
            if path in {"./", "."}:
                continue
            target = (source.parent / path).resolve()
            if not target.exists():
                line = text.count("\n", 0, match.start()) + 1
                missing.append(f"{source}:{line}: {reference}")

    for problem in missing:
        print(f"missing: {problem}", file=sys.stderr)
    if missing:
        return 1
    print(f"every local reference under {root} resolves")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(Path(sys.argv[1] if len(sys.argv) > 1 else "web")))
