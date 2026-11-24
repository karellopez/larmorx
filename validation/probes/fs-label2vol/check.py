# SPDX-License-Identifier: Apache-2.0
"""mri_label2vol against FreeSurfer 8.2.0's official program, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example; its SKIP list names
the replica's defect handling).

Generates seeded cases with larmorx-freesurfer-volumes' example ``fs_label2vol_probe`` (command
lines with the files they read: templates of several types and grids, labels in tkregister and
scanner coordinates, a closed surface of 66 vertices with its thickness, defect numbers (NaN from
their bits), an annotation with its colour table, segmentations with and without one,
``register.dat`` and ``.lta`` files, a subject's folder; every option, path and failure), runs the
official ``mri_label2vol`` (``$FREESURFER_HOME/bin``, by name) on each case through the
oracle-output cache at one thread, every case in the same folder (``<inputs>/fs-label2vol/case``;
the layout keyed as an input, ``layout.txt``), with a 30 s limit; runs the port on the same cases
with ``--upstream-defects`` ``reproduce`` and ``fix``, and compares the lines (exit status, standard
output, standard error, every file the run wrote or changed and every folder it made; the folder as
``<DIR>``, a written volume decompressed and each command line it records masked as ``<CMD>``):
``reproduce`` must give FreeSurfer's on every case but those where FreeSurfer crashes, takes its
result from memory contents or never ends (``SKIP``), which are counted, not compared; ``fix`` may
differ only where a defect it corrects changed the result (its notices). Prints the counts and the
hash the port's recorded test checks (``tests/fs_label2vol_official.rs``).

Every case runs with ``SUBJECTS_DIR`` the case's folder and ``FREESURFER_SEED=1234`` (the defects'
colour table takes random colours) unless it says otherwise. The official side runs with
``FS_DISABLE_LANG=1``. No case sets ``DIAG_NO`` or any diagnostic level: FreeSurfer's licence
check prints the licence's lines when the diagnostic number is above 0, and the script refuses
such a case.

    python -P validation/probes/fs-label2vol/check.py --seed 1 --n 500 [--json out.json]
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

# FreeSurfer's result is undefined: CRASH (the port's status is one FreeSurfer's build reaches by
# a signal: an abort, a division by 0, a crash; the official run's unflushed standard output is
# lost); MEMORY (FreeSurfer's result comes from memory contents: FS-53, FS-249, FS-1252, FS-1256,
# FS-1258, FS-1259);
# HANG (FS-1257: FreeSurfer never ends)
SKIP = ("CRASH", "MEMORY", "HANG")

#: ``--new-aseg2vol``'s time (``MRIseg2SegPVF(): done t = <seconds>, ...``): masked in the lines
#: and volatile in the cache
TIME = re.compile(r"(?<=MRIseg2SegPVF\(\): done t = )[^,]*(?=,)")

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
EXAMPLE = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_label2vol_probe"


def escape(b: bytes) -> str:
    return "".join(chr(c) if 0x20 <= c <= 0x7E and c != 0x5C else f"\\x{c:02x}" for c in b)


def fnv(b: bytes) -> int:
    h = 0xCBF29CE484222325
    for c in b:
        h = ((h ^ c) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def unhex(h: str) -> bytes:
    return b"" if h == "@" else bytes.fromhex(h)


def collect(d: Path, prefix: str = "") -> list[tuple[str, bytes | None]]:
    out: list[tuple[str, bytes | None]] = []
    for e in sorted(d.iterdir(), key=lambda p: p.name.encode()):
        n = prefix + e.name
        if e.is_dir():
            out.append((n, None))
            out.extend(collect(e, n + "/"))
        else:
            out.append((n, e.read_bytes()))
    return out


def canon(b: bytes) -> bytes:
    """A written volume as the probe compares it: decompressed when gzip, then each command line
    it records (the 8-byte length before ``ProgramName: `` to the NUL after it) as ``<CMD>``; a
    NIfTI-1 file's voxel offset and extension size masked too, its padding zeros left out (they
    follow the lines' lengths: the user's and the machine's names)."""
    if b[:2] == b"\x1f\x8b":
        try:
            b = gzip.decompress(b)
        except (OSError, EOFError):
            b = b""
    if len(b) >= 352 and b[:4] == (348).to_bytes(4, "little") and b[344:348] == b"n+1\0":
        v = struct.unpack("<f", b[108:112])[0]
        vox = 352 if v != v else len(b) if v > len(b) else max(int(v), 352)
        out = b[:108] + b"<VOX>" + b[112:352]
        if vox >= 360:
            out += b"<ESIZE>" + mask_command_lines(b[356:vox]).rstrip(b"\0")
        return out + b[vox:]
    return mask_command_lines(b)


def mask_command_lines(b: bytes) -> bytes:
    """Each command line (the 8-byte length before ``ProgramName: `` to the NUL after it) as
    ``<CMD>``."""
    out = bytearray()
    i = 0
    key = b"ProgramName: "
    while i < len(b):
        if b.startswith(key, i) and len(out) >= 8:
            del out[-8:]
            out += b"<CMD>"
            k = b.find(b"\0", i)
            i = len(b) if k < 0 else k
            continue
        out.append(b[i])
        i += 1
    return bytes(out)


def written(d: Path, inputs: dict[str, bytes], dirs: list[str]) -> str:
    out = []
    for n, b in collect(d):
        if b is None:
            if n not in dirs and not any(f.startswith(n + "/") for f in inputs):
                out.append(f"D {n}")
            continue
        if inputs.get(n) == b:
            continue
        if n.endswith((".txt", ".dat", ".label")):
            b = b.replace(str(d).encode(), b"<DIR>")
            out.append(f"F {n} {escape(b)}")
        else:
            b = canon(b).replace(str(d).encode(), b"<DIR>")
            out.append(f"F {n} {fnv(b):016x} {len(b)}")
    return " ".join(out)


#: The probe's folder layout, keyed as an input of every official run.
LAYOUT = "fs-label2vol: every case runs in <inputs>/case\n"


def layout(root: Path) -> Path:
    p = root / "layout.txt"
    if not p.is_file() or p.read_text(encoding="utf-8") != LAYOUT:
        p.write_text(LAYOUT, encoding="utf-8")
    return p


def official_line(exe: Path, line: str, root: Path, k: int) -> str:
    t = line.split()
    assert t[0] == "cat"
    d = (root / "case").resolve()
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", **freesurfer_env()}  # fmt: skip
    if t[1] != "-":
        for a in unhex(t[1]).decode().split(";"):
            if a.startswith("-"):
                env.pop(a[1:], None)
                continue
            key, _, val = a.partition("=")
            env[key] = val.replace("@DIR@", str(d))
    n = int(t[2])
    i = 3
    inputs, given = [], {}
    for _ in range(n):
        name, b = unhex(t[i]).decode(), unhex(t[i + 1])
        p = d / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(b)
        inputs.append(p)
        given[name] = b
        i += 2
    ndirs = int(t[i])
    dirs = [unhex(h).decode() for h in t[i + 1 : i + 1 + ndirs]]
    for x in dirs:
        (d / x).mkdir(parents=True, exist_ok=True)
    i += 1 + ndirs
    nargs = int(t[i])
    args = [unhex(h).decode() for h in t[i + 1 : i + 1 + nargs]]
    # never a run whose licence check prints the licence's lines (a diagnostic number above 0)
    if any(x.startswith("DIAG") for x in env):
        raise SystemExit(f"case {k}: DIAG_NO would print the licence's lines")
    if any(a.lower() == "--gdiagno" for a in args):
        raise SystemExit(f"case {k}: a diagnostic level")
    done = oracle_cache.run(
        [exe.name, *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(
            threads=1, family="freesurfer", formats="freesurfer", volatile=(TIME.pattern,)
        ),
        timeout=30,
        runner="fs-label2vol-probe",
        inputs=[*inputs, layout(root)],
    )
    rc = done.returncode
    status = "TIMEOUT" if rc is None else f"EXIT {rc & 0xFF}" if rc >= 0 else f"CRASH {-rc}"
    dd = escape(str(d).encode())

    def mask(s: str | bytes) -> str:
        b = s.encode("utf-8", "surrogateescape") if isinstance(s, str) else s
        return TIME.sub("<T>", escape(b).replace(dd, "<DIR>"))

    out = done.stdout_bytes
    files = written(d, given, dirs)
    return f"{status} | {mask(out)} | {mask(done.stderr_bytes)} | {files}"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=500)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--dump", type=Path, help="a folder for the official, reproduce, fix lines")
    ap.add_argument("--show", type=int, default=5, help="differing cases to print")
    ap.add_argument(
        "--golden", action="store_true", help="the golden cases: their official SHA-256 only"
    )
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_label2vol"
    if not exe.is_file():
        raise SystemExit(f"no {exe}")
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_label2vol_probe", "-j", str(a.jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )  # fmt: skip
    inputs = ws / "oracles" / "inputs" / "fs-label2vol"
    inputs.mkdir(parents=True, exist_ok=True)
    tag = "golden" if a.golden else f"{a.seed}-{a.n}"
    cases_path = inputs / f"cases-{tag}.txt"
    gen = ["golden-cases"] if a.golden else ["cases", str(a.seed), str(a.n)]
    text = subprocess.run([EXAMPLE, *gen], check=True, capture_output=True, text=True).stdout
    cases_path.write_text(text, encoding="utf-8")
    cases = text.splitlines()
    root = inputs
    official = [official_line(exe, c, root, k) for k, c in enumerate(cases)]
    shutil.rmtree(root / "case")
    if a.golden:
        h = hashlib.sha256()
        for c in official:
            h.update(c.encode() + b"\n")
        print(f"golden OFFICIAL_SHA256 {h.hexdigest()} ({len(official)} cases)")

    def port_lines(mode: str) -> list[tuple[str, list[str]]]:
        out = subprocess.run(
            [EXAMPLE, "run", cases_path, mode], check=True, capture_output=True, text=True
        ).stdout
        rows = []
        for line in out.splitlines():
            body, _, ids = line.partition("\t")
            rows.append((body, [x for x in ids.split(",") if x]))
        return rows

    rep, fix = port_lines("reproduce"), port_lines("fix")
    if a.dump:
        a.dump.mkdir(parents=True, exist_ok=True)
        (a.dump / "official.txt").write_text("\n".join(official) + "\n", encoding="utf-8")
        for name, rows in (("reproduce", rep), ("fix", fix)):
            text = "\n".join(f"{b}\t{','.join(i)}" for b, i in rows) + "\n"
            (a.dump / f"{name}.txt").write_text(text, encoding="utf-8")
    if not len(official) == len(rep) == len(fix) == len(cases):
        raise SystemExit(
            f"line counts differ: official {len(official)}, reproduce {len(rep)}, fix {len(fix)}"
        )
    equal = differ = skipped = skipped_same = 0
    fix_differ, fix_unexplained, fix_ids, rep_ids = 0, 0, Counter(), Counter()
    hashed = hashlib.sha256()
    first = []
    for k, (case, c, (r, ids), (f, fids)) in enumerate(zip(cases, official, rep, fix, strict=True)):
        for x in ids:
            rep_ids[x] += 1
        if any(x in ids for x in SKIP):
            skipped += 1
            skipped_same += c == r
            hashed.update(b"SKIP\n")
            continue
        hashed.update(c.encode() + b"\n")
        if c == r:
            equal += 1
        else:
            differ += 1
            if len(first) < a.show:
                first.append(
                    {"case": k, "args": case[-300:], "official": c[:1500], "port": r[:1500]}
                )
        if f != c:
            fix_differ += 1
            for x in fids:
                fix_ids[x] += 1
            if not fids:
                fix_unexplained += 1
                if len(first) < a.show:
                    first.append({"case": k, "official": c[:900], "fix": f[:900]})
    summary = {
        "seed": a.seed,
        "cases": len(cases),
        "reproduce_equal": equal,
        "reproduce_differ": differ,
        "skipped": skipped,
        "skipped_but_identical": skipped_same,
        "reproduce_notices": dict(rep_ids),
        "fix_differ": fix_differ,
        "fix_differ_without_notice": fix_unexplained,
        "fix_notices": dict(fix_ids),
        "official_sha256_skip": hashed.hexdigest(),
        "first_differ": first,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 1 if differ or fix_unexplained else 0


if __name__ == "__main__":
    raise SystemExit(main())
