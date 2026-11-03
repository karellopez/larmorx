# SPDX-License-Identifier: Apache-2.0
"""Compares larmorx-nc-core's Kiss FFT with Armadillo 14.2.1's (fslmaths' FFT convolution), the
twiddle factors apart from the transform's arithmetic.

Runs, for every transform length given:

1. ``fft_twiddles`` (``fft_twiddles.cc``): Armadillo's engine's factors, and glibc's ``cexp``,
   ``sincos``, ``cos`` and ``sin`` on the same angles; against the replica's factors (the ignored
   test ``twiddles`` of ``crates-nc/larmorx-nc-core/tests/fft_probe.rs``: correctly rounded
   ``cos`` and ``sin``);
2. ``fft_probe`` (``fft_probe.cc``): Armadillo's ``fft`` and ``ifft`` of seeded vectors; against
   the replica's engine (``kiss_fft_lines``) and against the replica's engine handed Armadillo's
   own factors (``kiss_fft_lines_with_twiddles``).

Prints counts only (and the first differing factors). Run inside a memory box
(``scripts/orchestration/boxed.sh``) from the repository's root, with ``cargo`` on the path. See
README.md.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def _cargo_test(name: str, sizes: str, extra_env: dict[str, str]) -> list[str]:
    env = {**os.environ, "LARMORX_FFT_SIZES": sizes, **extra_env}
    r = subprocess.run(
        [
            "cargo",
            "test",
            "--release",
            "--locked",
            "-p",
            "larmorx-nc-core",
            "--test",
            "fft_probe",
            "--",
            "--ignored",
            "--nocapture",
            "--exact",
            name,
        ],
        cwd=REPO / "crates-nc",
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    return r.stdout.splitlines()


def _parse_size_list(text: str) -> list[int]:
    out: list[int] = []
    for part in text.replace(",", " ").split():
        if "-" in part:
            a, b = part.split("-")
            out.extend(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--twiddles-probe", type=Path, required=True)
    ap.add_argument("--fft-probe", type=Path, required=True)
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--sizes", default="2-130", help="lengths, e.g. '2-130' or '15 30 60'")
    a = ap.parse_args()
    sizes = _parse_size_list(a.sizes)
    words = " ".join(str(n) for n in sizes)
    a.work.mkdir(parents=True, exist_ok=True)

    # 1. the twiddle factors
    tw_path = a.work / "arma_twiddles.txt"
    tw = subprocess.run(
        [str(a.twiddles_probe), *map(str, sizes)], capture_output=True, text=True, check=True
    ).stdout
    tw_path.write_text(tw, encoding="utf-8")
    arma: dict[tuple[int, int, int], list[str]] = {}
    for line in tw.splitlines():
        w = line.split()
        arma[(int(w[1]), int(w[2]), int(w[3]))] = w[4:]
    rep: dict[tuple[int, int, int], list[str]] = {}
    for line in _cargo_test("twiddles", words, {}):
        w = line.split()
        if not w or w[0] != "tw":
            continue
        rep[(int(w[1]), 0 if float(w[2]) < 0 else 1, int(w[3]))] = w[4:]
    assert arma.keys() == rep.keys(), "the same factors on both sides"
    counts = {"replica": 0, "cexp": 0, "sincos": 0, "cos,sin": 0, "angle": 0}
    sizes_differ: dict[int, int] = {}
    first: list[str] = []
    for key, (y, ar, ai, cr, ci, sc, ss, c, s) in sorted(arma.items()):
        ry, rc, rs = rep[key]
        n, inv, i = key
        zero = int(y, 16) & 0x7FFFFFFFFFFFFFFF == 0
        # the replica's factor: (1, angle) at a zero angle, as glibc's cexp gives
        rre, rim = ("3ff0000000000000", y) if zero else (rc, rs)
        counts["angle"] += ry != y
        differ = (ar, ai) != (rre, rim)
        counts["replica"] += differ
        counts["cexp"] += (ar, ai) != (cr, ci)
        counts["sincos"] += (ar, ai) != (sc, ss)
        counts["cos,sin"] += (ar, ai) != (c, s)
        if differ:
            sizes_differ[n] = sizes_differ.get(n, 0) + 1
            if len(first) < 8:
                first.append(
                    f"N={n} {'inv' if inv else 'fwd'} i={i}: arma {ar} {ai} replica {rre} {rim}"
                )
    total = len(arma)
    print(f"twiddles: {len(sizes)} lengths, {total} factors")
    print(f"  angles differ: {counts['angle']}")
    print(
        f"  Armadillo vs glibc cexp: {counts['cexp']} differ; vs sincos: {counts['sincos']}; "
        f"vs separate cos and sin: {counts['cos,sin']}"
    )
    print(
        f"  Armadillo vs replica (correctly rounded): {counts['replica']} differ, in "
        f"{len(sizes_differ)} of {len(sizes)} lengths"
    )
    print(f"  lengths with a differing factor: {' '.join(map(str, sorted(sizes_differ)))}")
    for f in first:
        print(f"    {f}")

    # 2. the transforms
    arma_lines = subprocess.run(
        [str(a.fft_probe), *map(str, sizes)], capture_output=True, text=True, check=True
    ).stdout.splitlines()

    def keep(lines: list[str]) -> list[str]:
        return [x for x in lines if len(x.split()) == 6 and x.split()[2] in ("fwd", "inv")]

    own = keep(_cargo_test("kiss_fft_lines", words, {}))
    given = keep(
        _cargo_test("kiss_fft_lines_with_twiddles", words, {"LARMORX_FFT_TWIDDLES": str(tw_path)})
    )
    assert len(arma_lines) == len(own) == len(given), (len(arma_lines), len(own), len(given))

    def compare(lines: list[str]) -> tuple[int, list[int]]:
        bad = [x for x, y in zip(arma_lines, lines, strict=True) if x != y]
        return len(bad), sorted({int(x.split()[0]) for x in bad})

    d_own, n_own = compare(own)
    d_given, n_given = compare(given)
    print(f"transforms: {len(arma_lines)} output values")
    print(
        f"  replica's engine: {d_own} differ, in {len(n_own)} lengths: "
        f"{' '.join(map(str, n_own)) or '-'}"
    )
    print(
        f"  with Armadillo's factors: {d_given} differ, in lengths "
        f"{' '.join(map(str, n_given)) or '-'}"
    )
    with_factor = set(sizes_differ)
    print(
        f"  lengths whose transforms differ without a differing factor: "
        f"{' '.join(str(n) for n in n_own if n not in with_factor) or '-'}"
    )
    return 0 if d_given == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
