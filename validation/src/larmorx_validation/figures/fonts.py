# SPDX-License-Identifier: Apache-2.0
"""The documentation site's fonts, for the figures: Instrument Serif, Inter and JetBrains Mono.

The figures are SVG with their text drawn as glyph outlines, so they show the site's typefaces
on any machine without loading a web font. The fonts are fetched once from Google Fonts' own
repository at a pinned commit (all three under the SIL Open Font License 1.1), checked against
their SHA-256, and kept in the figures cache (``<cache>/_fonts``). Inter and JetBrains Mono are
variable fonts; the weights the figures use are cut from them with fontTools' instancer.
"""

from __future__ import annotations

import hashlib
import urllib.request
from pathlib import Path

#: google/fonts at this commit (2026-10-16).
COMMIT = "47d54ca1ebb14fd864695f1d39dc734f39017c33"
BASE = f"https://raw.githubusercontent.com/google/fonts/{COMMIT}/"

#: file in the repository -> SHA-256
SOURCES: dict[str, str] = {
    "ofl/instrumentserif/InstrumentSerif-Regular.ttf": (
        "498efd461f6ddfcb7a111bf9a565709d2085d48201d501ead960d93e84ffbb88"
    ),
    "ofl/instrumentserif/InstrumentSerif-Italic.ttf": (
        "08939b8bdf534afec24ae0ef5e03f948940cd9a8fe08e7fecbad040e62327385"
    ),
    "ofl/inter/Inter[opsz,wght].ttf": (
        "29160a80ff49ddcab2c97711247e08b1fab27a484a329ce8b813d820dc559031"
    ),
    "ofl/jetbrainsmono/JetBrainsMono[wght].ttf": (
        "48715a42ec242c21e9f02692891e147d022299a52e48d5e413e1a942193ffeda"
    ),
}

#: The static instances the figures use: file name -> (source, axis values).
INSTANCES: dict[str, tuple[str, dict[str, float]]] = {
    "Inter-Regular.ttf": ("ofl/inter/Inter[opsz,wght].ttf", {"wght": 400, "opsz": 14}),
    "Inter-Medium.ttf": ("ofl/inter/Inter[opsz,wght].ttf", {"wght": 500, "opsz": 14}),
    "Inter-SemiBold.ttf": ("ofl/inter/Inter[opsz,wght].ttf", {"wght": 600, "opsz": 14}),
    "JetBrainsMono-Regular.ttf": ("ofl/jetbrainsmono/JetBrainsMono[wght].ttf", {"wght": 400}),
}


def _fetch(rel: str, dest: Path) -> Path:
    path = dest / Path(rel).name
    if path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == SOURCES[rel]:
        return path
    url = BASE + rel.replace("[", "%5B").replace("]", "%5D")
    with urllib.request.urlopen(url, timeout=120) as response:
        data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != SOURCES[rel]:
        raise RuntimeError(f"{url}: SHA-256 {digest}, expected {SOURCES[rel]}")
    dest.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    return path


def ensure(cache: Path) -> list[Path]:
    """The font files the figures use, fetched and cut into ``cache/_fonts`` if missing."""
    from fontTools.ttLib import TTFont
    from fontTools.varLib import instancer

    dest = cache / "_fonts"
    files = []
    for rel in SOURCES:
        path = _fetch(rel, dest)
        if "[" not in path.name:
            files.append(path)
    for name, (rel, axes) in INSTANCES.items():
        out = dest / name
        if not out.is_file():
            font = TTFont(_fetch(rel, dest))
            static = instancer.instantiateVariableFont(font, axes, updateFontNames=True)
            static.save(out)
        files.append(out)
    return files
