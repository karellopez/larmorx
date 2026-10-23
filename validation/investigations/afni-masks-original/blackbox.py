# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of AFNI 25.2.09's 3dAutomask and 3dUnifize beside larmorx's clean-room
original (and the replica), as the clean-room implementer used them to settle what the specs
left open. Nothing here reads AFNI's source or the replica's: the programs are run, their exit
status, messages and output bytes compared.

    python -P blackbox.py phantoms <work>                 # the inputs, into <work>/data
    python -P blackbox.py run <work> [--env A=B]... [--threads N] [--cwd DIR] <prog> [args...]
    python -P blackbox.py compare <work> <tool> <outputs> [--env A=B]... [--mode fix|reproduce]
                          [--threads N] [--label L] [--file NAME=TEXT]... -- [args...]
    python -P blackbox.py unifize-batch <work> [case...]  # `compare` on a list of 3dUnifize runs

<prog> is an AFNI program's name, `lx:3dX` (the original: `larmorx afni 3dX` with
LARMORX_IMPLEMENTATION=original) or `gpl:3dX` (the replica, for the wording of larmorx's own
lines). An argument starting with `@/` is a path under <work>. `compare` runs AFNI and the
original (by default in `reproduce` mode) in fresh directories under <work>/cmp/<label>, prints
`SAME <label>` or `DIFF <label>: ...` with the first difference (exit status, standard output,
standard error line by line with AFNI's version banner and larmorx's own lines set aside, and
each named output's header bytes and values).

Programs: AFNI in $AFNI_BIN (default <workspace>/oracles/afni-25.2.09/bin), larmorx in $LX_BIN
(default <repo>/target/release/larmorx), the replica in $LARMORX_GPL_BIN (default
<repo>/crates-gpl/target/release/larmorx-gpl); <workspace> is $LARMORX_WORKSPACE, else the
repository's parent directory. Run from the repository with `PYTHONPATH=python:validation/src`
(the phantoms are the parity suites').
"""

from __future__ import annotations

import gzip
import os
import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = Path(os.environ.get("AFNI_BIN", WORKSPACE / "oracles/afni-25.2.09/bin"))
LX = os.environ.get("LX_BIN", str(REPO / "target/release/larmorx"))
GPL = os.environ.get("LARMORX_GPL_BIN", str(REPO / "crates-gpl/target/release/larmorx-gpl"))


def _base_env(work: Path, threads: str) -> dict[str, str]:
    home = work / "home"
    home.mkdir(parents=True, exist_ok=True)
    return {
        "HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "AFNI_DONT_LOGFILE": "YES",
        "AFNI_NIFTI_NOEXT": "YES",
        "OMP_NUM_THREADS": threads,
    }


def _expand(args: list[str], work: Path) -> list[str]:
    return [str(work) + "/" + a[2:] if a.startswith("@/") else a for a in args]


def _command(prog: str) -> tuple[list[str], dict[str, str]]:
    if prog.startswith("gpl:"):
        return [GPL, "afni", prog[4:]], {}
    if prog.startswith("lx:"):
        return [LX, "afni", prog[3:]], {"LARMORX_IMPLEMENTATION": "original"}
    return [prog if os.path.isabs(prog) else str(AFNI / prog)], {}


# The inputs ------------------------------------------------------------------------------------


def phantoms(work: Path) -> None:
    import nibabel as nib
    from larmorx_validation.parity import afni_phantoms as ph

    out = work / "data"
    out.mkdir(parents=True, exist_ok=True)
    # The parity suites' synthetic inputs.
    for name in (
        "head-f32", "head-t2-f32", "head-i16", "head-u8", "head-i16-slope2", "head-i16-slope1",
        "head-i16-intercept", "head-f32-slope-inter", "head-f64", "head-i8", "head-u16",
        "head-i32", "head-nan", "head-oblique", "head-nearly-plumb", "head-qform-only",
        "head-sform-only", "head-las", "head-superior-first", "head-coronal", "head-metre",
        "series-i16", "series-f32", "series-negative", "series-slope", "series-one", "slice",
        "slice-small", "row", "two-slices", "tiny", "zeros", "constant",
    ):  # fmt: skip
        p = ph.synthetic(name)
        shutil.copy(p, out / p.name)
    t1 = ph.head("t1")
    grid = ph._affine(ph.GRID)

    def save(name, data, dtype, affine=grid, slope=None, **fields):
        img = nib.Nifti1Image(np.ascontiguousarray(np.asarray(data).astype(dtype)), affine)
        if slope is not None:
            img.header.set_slope_inter(slope, 0.0)
        for k, v in fields.items():
            img.header[k] = v
        if np.ndim(data) == 4:
            img.header["pixdim"][4] = 2.0
            img.header.set_xyzt_units("mm", "sec")
        nib.save(img, out / name)

    # Unit and other scale factors, and timing fields of a series.
    save("f32-slope1.nii", t1, np.float32, slope=1.0)
    save("u8-slope1.nii", np.round(t1 * 255 / t1.max()), np.uint8, slope=1.0)
    save("f32-slope2.nii", t1 / 2, np.float32, slope=2.0)
    s = ph._series(4)
    save("series4-slicecode.nii", s, np.float32, slice_code=1, slice_start=0, slice_end=39,
         slice_duration=0.05, dim_info=48)  # fmt: skip
    save("series4-toffset.nii", s, np.float32, toffset=0.75)
    # Small heads (3dUnifize is slow in AFNI at the full grid), a series for -EPI, a long one.
    for name, contrast, shape, seed in (
        ("small-t1", "t1", (32, 32, 20), 21),
        ("small-t2", "t2", (32, 32, 20), 22),
        ("mid-t1", "t1", (48, 44, 30), 23),
    ):
        save(f"{name}.nii", ph.head(contrast, shape, seed), np.float32, ph._affine(shape, 4.0))
    base = ph.head("t2", (32, 32, 20), 24)
    rng = np.random.default_rng(5)
    s = np.stack([base * (1 + 0.01 * t) + rng.normal(0, 6, base.shape) for t in range(5)], -1)
    save("small-series.nii", np.round(s), np.int16, ph._affine((32, 32, 20), 4.0))
    base = ph.head("t2", (24, 24, 16), 31)
    rng = np.random.default_rng(6)
    s = np.stack([base * (1 + 0.002 * t) + rng.normal(0, 6, base.shape) for t in range(150)], -1)
    save("long-series.nii", s, np.float32, ph._affine((24, 24, 16), 4.0))
    # Negative values.
    small = ph.head("t1", (32, 32, 20), 21).astype(np.float32)
    neg = small.copy()
    neg[small < 60] = -40.0
    neg[16, 16, 10] = -2000.0
    save("small-neg.nii", neg, np.float32, ph._affine((32, 32, 20), 4.0))
    save("small-allneg.nii", -small, np.float32, ph._affine((32, 32, 20), 4.0))
    # Odd shapes: flat along one axis, a 5-voxel cube, a strip, three planes.
    t1 = t1.astype(np.float32)
    for name, data in {
        "flat-y": t1[:, 32:33, :],
        "flat-x": t1[32:33, :, :],
        "cube5": t1[28:33, 28:33, 18:23],
        "strip": t1[:, 30:34, 18:22],
        "thin-z3": t1[:, :, 18:21],
    }.items():
        save(f"{name}.nii", data, np.float32)
    print(out)


# Running and comparing -------------------------------------------------------------------------


def _options(argv: list[str]) -> tuple[dict, list[str]]:
    o: dict = {"env": {}, "files": {}, "threads": None, "mode": "reproduce", "label": None,
               "cwd": None}  # fmt: skip
    while argv and argv[0].startswith("--") and argv[0] != "--":
        flag = argv.pop(0)
        if flag in ("--env", "--file"):
            k, v = argv.pop(0).split("=", 1)
            o["env" if flag == "--env" else "files"][k] = v
        else:
            o[flag[2:]] = argv.pop(0)
    if argv and argv[0] == "--":
        argv.pop(0)
    return o, argv


def run(work: Path, argv: list[str]) -> int:
    o, argv = _options(argv)
    cwd = Path(_expand([o["cwd"]], work)[0]) if o["cwd"] else Path.cwd()
    cwd.mkdir(parents=True, exist_ok=True)
    cmd, extra = _command(argv[0])
    env = {**_base_env(work, o["threads"] or "1"), **extra, **o["env"]}
    p = subprocess.run([*cmd, *_expand(argv[1:], work)], cwd=cwd, env=env, capture_output=True)
    sys.stdout.write("--- stdout ---\n" + p.stdout.decode("utf-8", "replace"))
    sys.stdout.write("--- stderr ---\n" + p.stderr.decode("utf-8", "replace"))
    print(f"[exit {p.returncode}]")
    return 0


BANNER = re.compile(
    r"^(?:[^\n]*::)?(?:\x1b\[7m)?\+\+ 3d\w+: AFNI version=[^\n]*\n|^\*\* Program compile date = [^\n]*\n",
    re.M,
)
ANSI = re.compile(r"\x1b\[[0-9;]*m")
ELAPSED = re.compile(r"Elapsed = [0-9.]+ sec")


def _own(line: str) -> bool:
    """larmorx's own lines (notices, its implementation line), which AFNI does not print."""
    plain = ANSI.sub("", line).rstrip()
    return (
        "upstream defect " in plain or plain.endswith("(larmorx)") or "++ implementation: " in plain
    )


def _nifti_parts(path: str) -> tuple[bytes, bytes]:
    raw = Path(path).read_bytes()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    if raw[:4] == b"\x5c\x01\x00\x00":
        off = int(np.frombuffer(raw[108:112], "<f4")[0])
    else:
        off = int(np.frombuffer(raw[168:176], "<i8")[0])
    return raw[:off], raw[off:]


def compare(work: Path, argv: list[str]) -> str:
    tool = argv.pop(0)
    outputs = [x for x in argv.pop(0).split(",") if x]
    o, argv = _options(argv)
    label = o["label"] or "case"
    base = work / "cmp" / label
    shutil.rmtree(base, ignore_errors=True)
    env = {**_base_env(work, o["threads"] or "2"), **o["env"]}
    runs = {}
    for who in ("afni", "lx"):
        d = base / who
        d.mkdir(parents=True)
        for name, text in o["files"].items():
            (d / name).write_text(text, encoding="utf-8")
        if who == "afni":
            cmd, e = [str(AFNI / tool)], dict(env)
        else:
            cmd = [LX, "afni", tool]
            e = dict(env, LARMORX_IMPLEMENTATION="original", LARMORX_UPSTREAM_DEFECTS=o["mode"])
        p = subprocess.run([*cmd, *_expand(argv, work)], cwd=d, env=e, capture_output=True)
        err = p.stderr.decode(errors="replace")
        if who == "afni":
            err = BANNER.sub("", err)
        else:
            err = "".join(x for x in err.splitlines(True) if not _own(x))
        runs[who] = (p.returncode, p.stdout.decode(errors="replace"),
                     ELAPSED.sub("Elapsed = N", err).replace(str(d), "<d>"), d)  # fmt: skip
    problems = []
    (ca, oa, ea, da), (cl, ol, el, dl) = runs["afni"], runs["lx"]
    if ca != cl:
        problems.append(f"exit AFNI {ca} larmorx {cl}")
    if oa != ol:
        problems.append("stdout differs")
    if ea != el:
        xa, xl = ea.splitlines(), el.splitlines()
        k = next((i for i, (x, y) in enumerate(zip(xa, xl, strict=False)) if x != y),
                 min(len(xa), len(xl)))  # fmt: skip
        a = xa[k] if k < len(xa) else None
        b = xl[k] if k < len(xl) else None
        problems.append(f"stderr line {k + 1}: AFNI {a!r} | larmorx {b!r}")
    for name in outputs:
        pa, pl = da / name, dl / name
        if pa.is_file() != pl.is_file():
            problems.append(f"{name}: written by AFNI {pa.is_file()}, by larmorx {pl.is_file()}")
            continue
        if not pa.is_file():
            continue
        (ha, va), (hl, vl) = _nifti_parts(str(pa)), _nifti_parts(str(pl))
        if ha != hl:
            at = [i for i in range(min(len(ha), len(hl))) if ha[i] != hl[i]]
            problems.append(
                f"{name}: header differs at bytes {at[:12]} (lengths {len(ha)} {len(hl)})"
            )
        if va != vl:
            import nibabel as nib

            a = np.asanyarray(nib.load(pa).dataobj.get_unscaled()).astype(np.float64)
            b = np.asanyarray(nib.load(pl).dataobj.get_unscaled()).astype(np.float64)
            if a.shape != b.shape:
                problems.append(f"{name}: shapes {a.shape} {b.shape}")
            else:
                d = np.abs(a - b)
                rel = float(d.max() / max(np.abs(a).max(), 1e-30))
                n = int(np.count_nonzero(a != b))
                problems.append(
                    f"{name}: {n}/{a.size} values differ, max |d| {d.max():.6g} (rel {rel:.2g})"
                )
    return f"DIFF {label}: " + "; ".join(problems) if problems else f"SAME {label}"


UNIFIZE_CASES = {
    "T2": "-T2 -prefix u.nii @/data/small-t2.nii",
    "T2T2": "-T2 -T2 -prefix u.nii @/data/small-t2.nii",
    "T2x3": "-T2 -T2 -T2 -prefix u.nii @/data/small-t2.nii",
    "GM": "-GM -prefix u.nii @/data/small-t1.nii",
    "GMT2T2": "-GM -T2 -T2 -prefix u.nii @/data/small-t2.nii",
    "verb": "-verb -prefix u.nii @/data/small-t1.nii",
    "T2verb": "-T2 -verb -prefix u.nii @/data/small-t2.nii",
    "quiet": "-quiet -prefix u.nii @/data/small-t1.nii",
    "epi": "-EPI -prefix u.nii @/data/small-series.nii",
    "epigm": "-GM -EPI -prefix u.nii @/data/small-series.nii",
    "epiq": "-EPI -quiet -prefix u.nii @/data/long-series.nii",
    "epilong": "-EPI -prefix u.nii @/data/long-series.nii",
    "epi3d": "-EPI -prefix u.nii @/data/small-t2.nii",
    "4d": "-prefix u.nii @/data/small-series.nii",
    "neg": "-prefix u.nii -amsave am.nii @/data/small-neg.nii",
    "allneg": "-prefix u.nii @/data/small-allneg.nii",
    "nosquash": "-nosquash -prefix u.nii @/data/small-t1.nii",
    "T2up95": "-T2 -T2up 95 -prefix u.nii @/data/small-t2.nii",
    "T2up100": "-T2 -T2up 100 -prefix u.nii @/data/small-t2.nii",
    "rbt": "-rbt 10 30 60 -prefix u.nii @/data/small-t1.nii",
    "urad5": "-Urad 5 -prefix u.nii @/data/small-t1.nii",
    "clfrac": "-clfrac 0.5 -prefix u.nii @/data/small-t1.nii",
    "slice": "-prefix u.nii @/data/slice.nii",
    "slicesmall": "-prefix u.nii @/data/slice-small.nii",
    "slicesmallT2": "-T2 -prefix u.nii @/data/slice-small.nii",
    "tiny": "-prefix u.nii @/data/tiny.nii",
    "zeros": "-prefix u.nii @/data/zeros.nii",
    "constant": "-prefix u.nii @/data/constant.nii",
    "two": "-prefix u.nii @/data/two-slices.nii",
    "useall": "-useall -prefix u.nii @/data/small-t1.nii",
    "input": "-input @/data/small-t1.nii -prefix u.nii",
    "flat-y": "-prefix u.nii @/data/flat-y.nii",
    "cube5": "-prefix u.nii @/data/cube5.nii",
    "thin-z3": "-prefix u.nii @/data/thin-z3.nii",
}


def unifize_batch(work: Path, names: list[str]) -> None:
    def one(name: str) -> str:
        argv = [
            "3dUnifize",
            "u.nii,am.nii",
            "--label",
            f"u-{name}",
            "--",
            *UNIFIZE_CASES[name].split(),
        ]
        return compare(work, argv)

    with ThreadPoolExecutor(3) as ex:
        for line in ex.map(one, names or list(UNIFIZE_CASES)):
            print(line, flush=True)


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[0] not in ("phantoms", "run", "compare", "unifize-batch"):
        print(__doc__, file=sys.stderr)
        return 2
    command, work, rest = argv[0], Path(argv[1]).resolve(), argv[2:]
    if command == "phantoms":
        phantoms(work)
    elif command == "run":
        return run(work, rest)
    elif command == "compare":
        print(compare(work, rest))
    else:
        unifize_batch(work, rest)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
