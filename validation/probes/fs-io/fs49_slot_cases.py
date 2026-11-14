# SPDX-License-Identifier: Apache-2.0
"""FS-49 at fs.io's callers (``freadInt`` at the end of a file), against FreeSurfer 8.2.0's code.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsio is a
black-box run of FreeSurfer's code).

Hand-made ``tr`` and ``ztr`` cases (``TAGreadStart`` on C streams and ``znzTAGreadStart`` on znzlib
files: a ``TAG_OLD_MGH_XFORM`` tag whose 32-bit length the file cuts after 0 to 2 bytes keeps the
tag's bytes, as does a 64-bit length cut after 4 or more) and ``ctr`` cases (the binary
colour table readers on C streams and one on a znzlib file: tables that end inside a run of
numbers read with nothing else between them, which keep the earlier number's bytes; tables that
end after an allocation, a name or a print, which read memory contents). Runs ``fsio`` through the
oracle-output cache and the port's example ``fs_io_probe`` in both modes and prints each case. A
line is ``SAME`` when ``reproduce`` gives FreeSurfer's line; ``PASS`` when every line is the same
or marked ``SKIP``/``VARY`` by the port (memory contents, FreeSurfer's out-of-bounds entries).

    python -P validation/probes/fs-io/fs49_slot_cases.py
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from check import oracle_cache, workspace  # noqa: E402

# (case, what it shows); hex numbers are big-endian as the files hold them
CASES = [
    ("tr 0 0000001e t", "the length cut at 0 bytes: the tag's (30, so 29)"),
    ("tr 0 0000001eabcd t", "cut at 2 bytes: ab cd, then the tag's 00 1e"),
    ("tr 0 0000001e12 t", "cut at 1 byte"),
    ("tr 0 0000001e000000 t t", "cut at 3 bytes, then the end"),
    ("tr 0 0000001e00000005414243440000001e t d 4 t", "a second tag after data"),
    ("ctr 0 1 0 0 fffffffe", "version 2, its entry count the version's bytes (-2)"),
    ("ctr 0 1 0 0 fffffffe0000", "the count cut at 2 bytes: 65534, then memory"),
    (
        "ctr 0 1 0 0 00000001000000010000000002780000000009",
        "version 1: the colour's last three numbers the first's (9)",
    ),
    (
        "ctr 0 1 0 0 000000010000000100000000027800000000090a0b",
        "version 1: the second number cut at 2 bytes",
    ),
    (
        "ctr 0 1 0 0 fffffffe0000000300000001000000000100000001000000026200000102030400",
        "version 2: the second number cut at 2 bytes, the others repeat it",
    ),
    (
        "ctr 0 1 0 0 fffffffe0000000300000001000000000100000001000000026200000102030400aabb",
        "version 2: the second number cut at 2 bytes (aa bb)",
    ),
    (
        "ctr 0 1 0 0 fffffffe000000030000000100000000020000000100000002620000000005000000060000000700000001",
        "version 2: the next structure number the last number read (1): a duplicate",
    ),
    (
        "ctr 0 1 0 0 fffffffe00000003000000010000000002",
        "version 2: the first structure number the entry count to read (2), then memory",
    ),
    ("ctr 0 1 0 0 fffffffe000000030000000100", "the count to read after a name: memory"),
    (
        "ctr 0 1 64 0 fffffffe0000000300000001000000000100000001000000026200000102030400",
        "with Gdiag's DIAG_SHOW (a print between two numbers)",
    ),
    (
        "ctr 1 1 0 0 fffffffe0000000300000001000000000100000001000000026200000102030400",
        "on a znzlib file (FS-167, as on a C stream since relay 26)",
    ),
    # relay 26: znzTAGreadStart (FS-167) on plain and compressed files: the tag-30 length and a
    # 64-bit length keep the tag's bytes
    ("ztr 0 0 0000001e t 0", "znz: tag 30's length cut at 0 bytes (29)"),
    ("ztr 1 0 0000001eabcd t 0", "znz, compressed: cut at 2 bytes"),
    ("ztr 1 0 00000003000000000000 t 0", "znz: a 64-bit length cut at 6 bytes: the tag's last two"),
    ("ztr 0 0 000000030000 t 0", "znz: a 64-bit length cut at 2 bytes: then memory"),
]


def main() -> int:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = check.probe_binary(ws)
    port = check.port_binary(6)
    path = ws / "oracles" / "inputs" / "fs-io" / "fs49-slot-cases.txt"
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
        print(f"{'SAME' if same else 'MARKED' if marked else 'DIFFERS'}  {what}  [{ids}]")
        if not same:
            print(f"  official  {official[i][:200]}")
            print(f"  reproduce {body[:200]}")
    print("FAIL" if bad else "PASS", bad)
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
