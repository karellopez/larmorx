# SPDX-License-Identifier: Apache-2.0
"""FS-167 at mghRead's readers (``znzreadInt``/``Float``/``Long``/``Double`` at the end of a file),
against FreeSurfer 8.2.0's code.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsio is a
black-box run of FreeSurfer's code).

Hand-made ``.mgh``/``.mgz`` files that end inside a run of numbers read with nothing else between
them (track A relay 42, measured in the official build's object code): mghRead's header (the six
sizes, the short, the fifteen geometry floats), the morph metadata tag (13), the second morph
geometry of tag 10, the morph labels (tag 12) and a frame of the frame tag (42), read by the
probe's ``mgf`` (``mghRead`` header only, every frame's fields printed). A number the file cuts
keeps the earlier number's bytes there (a float's reversed: ``znzreadFloat`` swaps them in place),
the first number of a run memory contents. Runs ``fsio`` through the oracle-output cache and the
port's example ``fs_io_probe`` in both modes and prints each case: ``SAME`` when ``reproduce`` gives
FreeSurfer's line, ``MARKED`` when the port marks it ``SKIP``/``VARY`` (a number of memory
contents read later in the case), ``PASS`` at the end when every line is one of the two; with
whether ``fix`` (the missing bytes 0) gives FreeSurfer's line too.

    python -P validation/probes/fs-io/mgh_slot_cases.py   (inside scripts/orchestration/boxed.sh)
"""

from __future__ import annotations

import gzip
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from check import oracle_cache, workspace  # noqa: E402


def i4(v: int) -> bytes:
    return struct.pack(">i", v)


def f4(v: float) -> bytes:
    return struct.pack(">f", v)


def i8(v: int) -> bytes:
    return struct.pack(">q", v)


def d8(v: float) -> bytes:
    return struct.pack(">d", v)


GEOM = [1.0, 1.5, 2.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 3.25, -7.5, 12.0]


def header(w: int, h: int, d: int, nf: int, ras: bool = True) -> bytes:
    b = i4(1) + i4(w) + i4(h) + i4(d) + i4(nf) + i4(3) + i4(0)
    b += struct.pack(">h", 1 if ras else 0)
    used = b"".join(f4(x) for x in GEOM) if ras else b""
    return b + used + bytes(254 - len(used))


def volume(w: int = 1, h: int = 1, d: int = 1, nf: int = 1) -> bytes:
    """A float volume's header, voxels and the five optional floats (tags may follow)."""
    data = b"".join(f4(0.5 + i) for i in range(w * h * d * nf))
    tail = f4(2000.0) + f4(0.5) + f4(30.0) + f4(900.0) + f4(256.0)
    return header(w, h, d, nf) + data + tail


def vg(valid: int, whd: tuple[int, int, int], name: bytes) -> bytes:
    """A geometry of tag 10 (no shears; the 512 name bytes)."""
    b = i4(valid) + b"".join(i4(x) for x in whd) + b"".join(f4(x) for x in GEOM)
    return b + name + bytes(512 - len(name))


def matrix_tag() -> bytes:
    text = b"Matrix" + b"".join(
        b" %10.6f" % x for x in [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    )
    return i4(0) + i8(1600) + text + bytes(1600 - len(text))


def frame(ftype: int, units: int = 7) -> bytes:
    b = i4(ftype) + b"".join(f4(x) for x in [30.0, 2000.0, 0.5, 900.0, 1.25, 4.0])
    b += i4(2) + f4(0.75) + f4(64.0) + b"".join(f4(x) for x in [1, 0, 0, 0, 1, 0, 0, 0, 1])
    b += i4(5) + b"frame one" + bytes(1024 - 9) + i4(3) + matrix_tag() + f4(0.25) + i4(units)
    if ftype == 1:
        b += b"".join(d8(x) for x in [0.1, 0.2, 0.3, 1.0, 2.0, 3.0, 1000.0, 7.0])
        for k in range(4):
            b += i8(100 + k) + i8(200 + k) + d8(0.5 + k)
    return b


def case(data: bytes, cut: int, gz: bool = False, rv: int = 0) -> str:
    raw = data[:cut]
    name = b"w/a.mgz" if gz else b"w/a.mgh"
    body = gzip.compress(raw, mtime=0) if gz else raw
    hexname = name.hex()
    return f"mgf 0 0 {rv} -1 x{hexname} F{hexname}:{len(body)}+0:{body.hex()}"


HDR = header(1, 1, 1, 1)
META = volume() + i4(13) + i8(12) + i4(7) + i4(2) + f4(5.0)
META_AT = len(volume()) + 12
GEOMS = volume() + i4(10) + vg(1, (4, 5, 6), b"image.mgz") + vg(1, (7, 8, 9), b"atlas.mgz")
G2_AT = len(volume()) + 4 + len(vg(1, (4, 5, 6), b"image.mgz"))
LABELS = volume(2, 1, 1) + i4(12) + i4(41) + i4(17)
LAB_AT = len(volume(2, 1, 1)) + 4
FRAMES = volume() + i4(42) + i8(len(frame(0))) + frame(0)
FR_UNITS = len(FRAMES) - 4
DIFF = volume() + i4(42) + i8(len(frame(1))) + frame(1)
DIFF_DX = len(volume()) + 12 + len(frame(0))

# (case, what it shows)
CASES = [
    (case(HDR, 8 + 2), "header: height cut at 2 bytes, then the sizes and the short (memory)"),
    (case(HDR, 30 + 4 + 2), "header: ysize cut at 2 bytes, the floats after it from the slot"),
    (case(HDR, 30 + 4 + 2, gz=True), "the same compressed"),
    (case(HDR, 30 + 12), "header: the floats after zsize from the slot (reversed by each read)"),
    (case(HDR, 30 + 12 + 3), "header: x_r cut at 3 bytes"),
    (case(HDR, 30 + 56), "header: c_s cut at 0 bytes"),
    (case(META, META_AT + 4 + 1), "tag 13: the spacing cut at 1 byte, exp_k from it"),
    (case(META, META_AT + 2, gz=True), "tag 13: the format cut at 2 bytes (memory), compressed"),
    (case(GEOMS, G2_AT + 6), "tag 10: the atlas geometry's width cut at 2 bytes"),
    (case(GEOMS, G2_AT + 16 + 8 + 1), "tag 10: the atlas geometry's zsize cut at 1 byte"),
    (case(GEOMS, len(volume()) + 4 + 30), "tag 10: the image geometry cut (the atlas's memory)"),
    (case(LABELS, LAB_AT + 4 + 2), "tag 12: the second label cut at 2 bytes"),
    (case(LABELS, LAB_AT + 4), "tag 12: the second label cut at 0 bytes"),
    (case(FRAMES, FR_UNITS + 2), "tag 42: units cut at 2 bytes"),
    (case(FRAMES, FR_UNITS), "tag 42: units cut at 0 bytes (thresh's bytes)"),
    (case(FRAMES, FR_UNITS - 2), "tag 42: thresh cut (the first after the matrix: memory)"),
    (case(FRAMES, len(volume()) + 12 + 4 + 8), "tag 42: TR cut at 0 bytes (then the dof: memory)"),
    (case(DIFF, len(DIFF)), "tag 42: a whole diffusion frame"),
    (case(DIFF, len(DIFF) - 30), "tag 42: a diffusion frame cut in D4_ramp (D4_amp: memory)"),
    (
        case(DIFF, DIFF_DX + 8 + 3),
        "tag 42: DY cut at 3 bytes: the doubles to TM from it as FreeSurfer (the longs: memory)",
    ),
    (case(DIFF, DIFF_DX + 56 + 5), "tag 42: TM cut at 5 bytes (then the longs' memory)"),
]


def main() -> int:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = check.probe_binary(ws)
    port = check.port_binary(6)
    path = ws / "oracles" / "inputs" / "fs-io" / "mgh-slot-cases.txt"
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [c for c, _ in CASES]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    done = oracle_cache.run(
        [exe, path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=600,
        runner="fs-io-probe",
    )
    official = done.stdout.splitlines()
    out = {
        mode: subprocess.run(
            [str(port), "run", str(path), mode], check=True, capture_output=True, text=True
        ).stdout.splitlines()
        for mode in ("reproduce", "fix")
    }
    bad = 0
    for i, (_case, what) in enumerate(CASES):
        body, _, ids = out["reproduce"][i].partition("\t")
        same = check.matches(official[i], body)
        marked = "SKIP" in ids or "VARY" in ids
        bad += not same and not marked
        fbody = out["fix"][i].partition("\t")[0]
        fix = "fix the same" if check.matches(official[i], fbody) else "fix differs"
        print(f"{'SAME' if same else 'MARKED' if marked else 'DIFFERS'}  {what}  [{ids}; {fix}]")
        if not same:
            print(f"  official  {official[i][:300]}")
            print(f"  reproduce {body[:300]}")
    print("FAIL" if bad else "PASS", bad)
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
