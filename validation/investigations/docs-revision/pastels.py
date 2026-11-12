# SPDX-License-Identifier: Apache-2.0
"""The software colours of the site's Programs page, checked (docs/writing-guide.md §5).

Each software has a pastel: a band behind its group on the Programs page and a chip with its
name, the same in both themes, its name always beside it. Today's six pastels (ANTs, AFNI, FSL,
LARMORX, FreeSurfer, Workbench) stay; tedana, sdcflows and fMRIPrep get one each. This script
checks the pastels of the site's `data/ecosystems.yml` in the Programs page's order (ANTs, AFNI,
FSL, tedana, sdcflows, fMRIPrep, LARMORX, FreeSurfer, Workbench), with the measures the
verdicts' colours were checked with (data/verdicts.yml's header: OKLab ΔE under normal vision
and under protanopia and deuteranopia, Machado, Oliveira and Fernandes 2009, severity 1; WCAG
2's contrast ratio):

- a new pastel and its neighbours: normal vision ΔE ≥ 10, protanopia and deuteranopia ΔE ≥ 9
  (today's closest neighbours with programs, AFNI and FSL, are 13.6, 10.3 and 12.4; the two
  planned groups, FreeSurfer and Workbench, 9.0, 5.9 and 8.7). Pastels are pale, so they cannot
  be as far apart as the verdicts' fills (≥ 18 and ≥ 11), and a group is never told by colour
  alone: its name is on its chip;
- a new pastel and every other pastel: normal vision ΔE ≥ 7, protanopia and deuteranopia
  ΔE ≥ 4 (the reviewer's round 5: the closest such pair, tedana and LARMORX, is 4.3 under
  protanopia);
- the chip's text, the site's `--on-pastel` ink #16130f, at 4.5:1 or more on every pastel, and
  the new deep variants (marks on the light paper) at 3:1 or more against the light paper
  (today's are printed for information: ANTs' is 2.997:1 and LARMORX's 2.998:1);
- the new pastels away from the ports' colours: their hue (OKLCh) at least 35° from the
  replica's blue and the clean room's apricot, and their chroma at least 0.05, so that none reads
  as the original's grey.

Every pair is printed under normal vision, protanopia, deuteranopia and tritanopia (for the
record); tritanopia and the pairs of today's pastels are information only. Three of today's
pairs are under 4 for colour blindness (ANTs and FreeSurfer, AFNI and LARMORX, FSL and
Workbench): they stay as they are, each always beside its name (the hero's legend, the chips).

    python -P validation/investigations/docs-revision/pastels.py <site checkout>

It prints one line per check and the word PASS or FAIL at the end.
"""

from __future__ import annotations

import itertools
import math
import sys
from pathlib import Path

import yaml

ORDER = [
    "ants",
    "afni",
    "fsl",
    "tedana",
    "sdcflows",
    "fmriprep",
    "larmorx",
    "freesurfer",
    "workbench",
]
NEW = ["tedana", "sdcflows", "fmriprep"]
PORTS = {
    "replica": "#467dea",
    "clean room": "#c76717",
    "replica, dark": "#5890fa",
    "clean room, dark": "#d97530",
}
ORIGINAL = ["#4a453e", "#b6b0a3"]
ON_PASTEL = "#16130f"
PAPER_LIGHT = "#e8e1d2"
CARD_LIGHT = "#f0ebe0"

MACHADO = {
    "protanopia": [
        [0.152286, 1.052583, -0.204868],
        [0.114503, 0.786281, 0.099216],
        [-0.003882, -0.048116, 1.051998],
    ],
    "deuteranopia": [
        [0.367322, 0.860646, -0.227968],
        [0.280085, 0.672501, 0.047413],
        [-0.011820, 0.042940, 0.968881],
    ],
    "tritanopia": [
        [1.255528, -0.076749, -0.178779],
        [-0.078411, 0.930809, 0.147602],
        [0.004733, 0.691367, 0.303900],
    ],
}


def lin(c: float) -> float:
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def rgb(hex_: str) -> tuple[float, float, float]:
    h = hex_.lstrip("#")
    return tuple(lin(int(h[i : i + 2], 16) / 255) for i in (0, 2, 4))


def oklab(lrgb) -> tuple[float, float, float]:
    r, g, b = lrgb
    l_ = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m_ = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s_ = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l_, m_, s_ = (math.copysign(abs(v) ** (1 / 3), v) for v in (l_, m_, s_))
    return (
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    )


def simulate(lrgb, kind: str):
    m = MACHADO[kind]
    return tuple(min(1.0, max(0.0, sum(m[i][j] * lrgb[j] for j in range(3)))) for i in range(3))


def delta_e(a, b) -> float:
    return 100 * math.dist(oklab(a), oklab(b))


def luminance(hex_: str) -> float:
    r, g, b = rgb(hex_)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a: str, b: str) -> float:
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def hue_chroma(hex_: str) -> tuple[float, float]:
    _, a, b = oklab(rgb(hex_))
    return math.degrees(math.atan2(b, a)) % 360, math.hypot(a, b)


def main(argv: list[str]) -> int:
    site = Path(argv[1])
    ecos = {
        e["id"]: e
        for e in yaml.safe_load((site / "data/ecosystems.yml").read_text(encoding="utf-8"))
    }
    missing = [k for k in ORDER if k not in ecos]
    if missing:
        print(f"FAIL: no pastel for {missing}")
        return 1
    ok = True

    def check(cond: bool, line: str) -> None:
        nonlocal ok
        ok &= cond
        print(("ok    " if cond else "FAIL  ") + line)

    def de(x: str, y: str, kind: str | None = None) -> float:
        a, b = rgb(ecos[x]["pastel"]), rgb(ecos[y]["pastel"])
        return delta_e(simulate(a, kind), simulate(b, kind)) if kind else delta_e(a, b)

    for x, y in itertools.pairwise(ORDER):
        new = x in NEW or y in NEW
        line = f"{x} / {y}: normal {de(x, y):.1f}, " + ", ".join(
            f"{k} {de(x, y, k):.1f}" for k in MACHADO
        )
        if not new:
            print("info  " + line)
            continue
        ok_pair = de(x, y) >= 10 and all(de(x, y, k) >= 9 for k in ("protanopia", "deuteranopia"))
        check(ok_pair, line + " (normal ≥ 10, protanopia and deuteranopia ≥ 9)")
    for x, y in itertools.combinations(ORDER, 2):
        line = f"{x} / {y}: normal {de(x, y):.1f}, " + ", ".join(
            f"{k} {de(x, y, k):.1f}" for k in MACHADO
        )
        if x not in NEW and y not in NEW:
            print("info  " + line + " (any two)")
            continue
        ok_pair = de(x, y) >= 7 and all(de(x, y, k) >= 4 for k in ("protanopia", "deuteranopia"))
        check(ok_pair, line + " (any two: normal ≥ 7, protanopia and deuteranopia ≥ 4)")
    for x in ORDER:
        p = ecos[x]["pastel"]
        c = contrast(ON_PASTEL, p)
        check(c >= 4.5, f"{x} {p}: the chip's ink {c:.1f}:1 (≥ 4.5)")
        if ecos[x].get("deep"):
            c = contrast(ecos[x]["deep"], PAPER_LIGHT)
            line = f"{x} deep {ecos[x]['deep']}: against the light paper {c:.2f}:1 (≥ 3)"
            if x in NEW:
                check(c >= 3, line)
            else:
                print("info  " + line)
    for x in NEW:
        h, ch = hue_chroma(ecos[x]["pastel"])
        for name in ("replica", "clean room"):
            ph, _ = hue_chroma(PORTS[name])
            gap = min(abs(h - ph), 360 - abs(h - ph))
            check(gap >= 35, f"{x}: hue {h:.0f}°, {gap:.0f}° from the {name}'s {ph:.0f}° (≥ 35)")
        check(ch >= 0.05, f"{x}: chroma {ch:.3f} (≥ 0.05, not the original's grey)")
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
