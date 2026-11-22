# SPDX-License-Identifier: Apache-2.0
"""mris_calc as a black box: FreeSurfer 8.2.0's official program against a candidate, case by case.

An open probe (``README.md``): it writes seeded synthetic inputs (MGZ volumes in ``float``,
``uchar``, ``short`` and ``int``, a three-frame one, one with the same number of values in other
dimensions, a smaller one, one on another grid, a NIfTI copy; curvature files with NaNs,
infinities, denormals and zeros, one with other face counts, one shorter than its count, one
with a negative count, one empty, one of another length, files that are neither; labels inside,
outside and without the values; a text file, a folder, a file whose name is a number) into
``<workspace>/oracles/inputs/fs-calc-prog/<seed>/``, runs each case's command line through the
official program (``$FREESURFER_HOME/bin/mris_calc``, found on the ``PATH`` by its name as the
scripts run it) by the oracle-output cache at one thread, and through the candidate
(``--candidate``, a command line whose ``{mode}`` becomes ``reproduce`` and then ``fix``), each
in a folder of its own (outputs are written there; ``=x`` copies an input there first, for the
operations that write over their input). Compared: the exit status, standard output and error
(the ``VmPeak`` figure, ``--all-info``'s time and the candidate's ``larmorx:`` notices aside) and
every file written (FreeSurfer's provenance aside: ``larmorx_validation.freesurfer.files``).
``reproduce`` must give FreeSurfer's on every case but those whose reproduce run notes a defect
of ``NOT_COMPARABLE``, which are counted; ``fix`` may differ only with a notice.

    python -P validation/investigations/freesurfer-mris-calc-probe/compare.py --seed 1 \\
        --candidate '<program> --upstream-defects {mode} freesurfer mris_calc'
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import struct
import subprocess
import sys
from collections import Counter
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer import files as fsfiles  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

#: The defects (``larmorx_core::defects``) whose cases are counted, not compared: FreeSurfer
#: reads memory it never wrote (a curvature file shorter than its count: FS-167; a thread count
#: that is no number: FS-1124) or writes outside its mask (FS-1125).
NOT_COMPARABLE = frozenset({"FS-167", "FS-1124", "FS-1125"})
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report holds the run's time,
#: the last line the process's peak memory.
POLICY = oracle_cache.Policy(
    threads=1,
    family="freesurfer",
    formats="freesurfer",
    volatile=(r"TimeStamp: \S+", r"VmPeak +-?\d+"),
)
STAMP = re.compile(r"TimeStamp: \S+")
VMPEAK = re.compile(r"(#VMPC# mris_calc VmPeak +)-?\d+")
#: A name of 65,536 bytes (past the program's text buffers) and one just short of it.
LONG = "x" * 65536
NEAR = "y" * 65525


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def curv(path: Path, values: list, nfaces: int = 7, vpv: int = 1, count: int | None = None,
         magic: bytes = b"\xff\xff\xff") -> Path:  # fmt: skip
    """A curvature file of the new format: the magic number, the count, the faces and values per
    vertex (big endian), then the values (a float, or an int as the value's bits)."""
    n = len(values) if count is None else count
    with open(path, "wb") as f:
        f.write(magic + struct.pack(">iii", n, nfaces, vpv))
        for v in values:
            f.write(struct.pack(">I", v) if isinstance(v, int) else struct.pack(">f", v))
    return path


def label(path: Path, vnos: list[int]) -> Path:
    lines = [f"#!ascii label  , from subject probe vox2ras=TkReg\n{len(vnos)}\n"]
    lines += [f"{v}  {v * 0.5:.3f}  {-v * 0.25:.3f}  {v * 0.125:.3f} {v * 0.01:.10f}\n"
              for v in vnos]  # fmt: skip
    path.write_text("".join(lines), encoding="utf-8")
    return path


def inputs(ws: Path, seed: int) -> dict[str, Path]:
    d = ws / "oracles" / "inputs" / "fs-calc-prog" / str(seed)
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(seed)
    aff = np.diag([1.0, 1.0, 1.0, 1.0])
    aff[:3, 3] = [-5.0, -6.0, -4.0]
    shape = (10, 12, 8)
    f: dict[str, Path] = {}
    v = rng.normal(0.0, 40.0, shape)
    v[rng.random(shape) < 0.1] = 0.0
    f["vf"] = save(d / "vf.mgz", v.astype(np.float32), aff)
    f["vf2"] = save(d / "vf2.mgz", rng.normal(5.0, 20.0, shape).astype(np.float32), aff)
    f["vu"] = save(d / "vu.mgz", rng.integers(0, 256, shape).astype(np.uint8), aff)
    f["vs"] = save(d / "vs.mgz", rng.integers(-500, 500, shape).astype(np.int16), aff)
    f["vi"] = save(d / "vi.mgz", rng.integers(-70000, 70000, shape).astype(np.int32), aff)
    f["v3"] = save(d / "v3.mgz", rng.normal(0.0, 3.0, shape + (3,)).astype(np.float32), aff)
    f["vother"] = save(d / "vother.mgz", rng.normal(0.0, 3.0, (12, 10, 8)).astype(np.float32),
                       aff)  # fmt: skip
    f["vsmall"] = save(d / "vsmall.mgz", rng.normal(0.0, 3.0, (5, 5, 5)).astype(np.float32), aff)
    aff2 = np.diag([1.5, 0.8, 1.2, 1.0])
    aff2[:3, 3] = [3.0, -2.0, 7.0]
    f["vgeo"] = save(d / "vgeo.mgz", rng.normal(1.0, 2.0, shape).astype(np.float32), aff2)
    f["mask"] = save(d / "mask.mgz", (rng.random(shape) < 0.4).astype(np.float32), aff)
    nib.save(nib.Nifti1Image(v.astype(np.float32), aff), str(d / "vf.nii.gz"))
    f["vnii"] = d / "vf.nii.gz"
    # surface values (a 1 x 1 x n volume, as label-cortex's masks)
    f["sv"] = save(d / "sv.mgz", rng.normal(0.0, 1.0, (300, 1, 1)).astype(np.float32), np.eye(4))
    specials = [0x7FA00007, 0xFFC00002, 0x7F800000, 0xFF800000, 1, 0x80000001, 0x00800000,
                0x80000000, 0x7FC00003, 0xFFA00009]  # fmt: skip
    a = list(rng.normal(0.0, 2.0, 190).astype(np.float32).tolist())
    a[::17] = [0.0] * len(a[::17])
    f["ca"] = curv(d / "lh.a", a + specials)
    b = list(rng.normal(1.0, 3.0, 190).astype(np.float32).tolist())
    b[5::13] = [0.0] * len(b[5::13])
    f["cb"] = curv(d / "lh.b", b + specials[::-1], nfaces=11, vpv=1)
    f["cpos"] = curv(d / "lh.pos", list(rng.uniform(0.1, 5.0, 200).astype(np.float32).tolist()))
    f["cs"] = curv(d / "lh.sig", list(rng.normal(0.0, 4.0, 200).astype(np.float32).tolist()))
    f["cint"] = curv(d / "lh.int", [float(x) for x in rng.integers(-9, 9, 200)])
    f["cshort"] = curv(d / "lh.short", [1.0, 2.0, 3.0], count=6)
    f["cneg"] = curv(d / "lh.neg", [1.0], count=-3)
    f["czero"] = curv(d / "lh.zero", [], count=0)
    f["cone"] = curv(d / "lh.one", [4.5])
    f["csmall"] = curv(d / "lh.small", [1.0, 2.0, 3.0, 4.0, 5.0])
    f["cext"] = curv(d / "lh.a.curv", a + specials)
    f["cold"] = curv(d / "lh.old", [1.0, 2.0], magic=b"\x00\x00\x02")
    (d / "notes.txt").write_text("not a volume\n", encoding="utf-8")
    f["txt"] = d / "notes.txt"
    (d / "folder").mkdir(exist_ok=True)
    f["dir"] = d / "folder"
    (d / "2").write_text("two\n", encoding="utf-8")
    f["two"] = d / "2"
    f["lab"] = label(d / "lh.in.label", [0, 3, 4, 17, 50, 199])
    f["labout"] = label(d / "lh.out.label", [1, 2, 205])
    f["labneg"] = label(d / "lh.neg.label", [-1, 5, 6])
    f["labempty"] = label(d / "lh.empty.label", [])
    f["labvol"] = label(d / "lh.vol.label", [0, 7, 100, 959])
    return f


def cases(f: dict[str, Path]) -> list[list[str]]:
    """The command lines (inputs by absolute name; ``=x``: a copy of input ``x`` in the case's
    folder, by its own name; outputs are relative)."""
    s = {k: str(v) for k, v in f.items()}
    binary = ["mul", "div", "mod", "add", "pow", "sub", "sub0", "sratio", "pctdiff", "pctdiff0",
              "sqd", "atan2", "bcor", "mag", "eq", "lt", "lte", "gt", "gte", "masked", "and", "or",
              "andbw", "orbw", "upperlimit", "lowerlimit"]  # fmt: skip
    unary = ["sqr", "sqrt", "not", "sig2p", "log10", "abs", "inv", "sign", "log", "norm"]
    stats = ["stats", "size", "min", "max", "mean", "std", "sum", "prod"]
    c: list[list[str]] = [
        [],
        ["-u"],
        ["-h"],
        ["-?"],
        ["--help"],
        ["--usage"],
        ["-help"],
        ["--version"],
        ["--version", s["ca"], "add", "1"],
        ["--all-info"],
        ["-v"],
        ["-verbosity", "1", s["ca"], "add", "1"],
        ["-x", s["ca"], "add", "1"],
        ["-", s["ca"], "add", "1"],
        ["--", "-o", "o.crv", s["ca"], "add", "1"],
        ["--foo", "-o", "o.crv", s["ca"], "add", "1"],
        ["-t", "-o", "o.crv", s["ca"], "add", "1"],
        ["-threads", "2", "-o", "o.crv", s["ca"], "add", "1"],
        ["-nthreads", "3", "-o", "o.crv", s["ca"], "add", "1"],
        ["-threads", "x", "-o", "o.crv", s["ca"], "add", "1"],
        ["--threads", "2", "-o", "o.crv", s["ca"], "add", "1"],
        ["-o"],
        [s["ca"], "add", "1", "-o"],
        ["-l"],
        ["--verbosity"],
        ["-threads"],
        [s["ca"]],
        [s["ca"], "add", "1", "2"],
        ["-o", "o.crv", s["ca"], "foo", "1"],
        ["-o", "o.crv", s["ca"], "mini"],
        ["-o", "o.crv", s["ca"], "maxi"],
        ["-o", "o.crv", s["ca"], "MUL", "2"],
    ]
    # every operation on two curvature files and on a curvature file and a number
    for op in binary:
        c.append(["-o", f"c_{op}.crv", s["ca"], op, s["cb"]])
    for op in ["mul", "div", "mod", "pow", "sub0", "sratio", "atan2", "bcor", "masked", "andbw",
               "eq", "set"]:  # fmt: skip
        c.append(["-o", f"n_{op}.crv", s["ca"], op, "-2.5"])
    for op in unary:
        c.append(["-o", f"u_{op}.crv", s["ca"], op])
    for op in ["pow", "bcor", "atan2", "mod"]:
        c.append(["-o", f"p_{op}.crv", s["cs"], op, s["cpos"]])
    for op in ["sig2p", "log10", "log", "sqrt", "inv"]:
        c.append(["-o", f"q_{op}.crv", s["cpos"], op])
        c.append(["-o", f"r_{op}.crv", s["cs"], op])
    for op in ["andbw", "orbw", "and", "or", "not"]:
        c.append(["-o", f"i_{op}.crv", s["cint"], op, *([] if op == "not" else [s["cs"]])])
    for op in stats:
        c.append([s["ca"], op])
        c.append([s["cs"], op])
    c += [
        ["-o", "st.txt", s["cs"], "stats"],
        ["-o", "mn.txt", s["cs"], "min"],
        ["-o", "mx.txt", s["cs"], "max"],
        ["-o", "me.txt", s["cs"], "mean"],
        ["-o", "sd.txt", s["cs"], "std"],
        ["-o", "su.txt", s["cs"], "sum"],
        ["-o", "pr.txt", s["cs"], "prod"],
        [s["cone"], "stats"],
        [s["cs"], "ascii"],
        ["-o", "vals.txt", s["ca"], "ascii"],
        ["-o", "nodir/vals.txt", s["ca"], "ascii"],
        ["-o", "nodir/st.txt", s["cs"], "max"],
        ["-o", "nodir/o.crv", s["ca"], "add", "1"],
        ["-o", "nodir/o.mgz", s["vf"], "add", "1"],
        # numbers as the second input
        ["-o", "f1.crv", s["ca"], "add", "nan"],
        ["-o", "f2.crv", s["ca"], "mul", "-inf"],
        ["-o", "f3.crv", s["ca"], "add", "1e50"],
        ["-o", "f4.crv", s["ca"], "add", "1e-50"],
        ["-o", "f5.crv", s["ca"], "add", "0x1p3"],
        ["-o", "f6.crv", s["ca"], "add", "nan(0x123)"],
        ["-o", "f7.crv", s["ca"], "add", " 5"],
        ["-o", "f8.crv", s["ca"], "add", "5x"],
        ["-o", "f9.crv", s["ca"], "add", ""],
        ["-o", "f10.crv", s["ca"], "add", "1e400"],
        ["-o", "f11.crv", s["ca"], "add", "0.1"],
        ["-o", "f12.crv", s["ca"], "add", s["two"]],
        ["-o", "f13.crv", "1.5", "add", s["ca"]],
        # the output's name
        [s["ca"], "add", "1"],
        ["-o", "plain", s["ca"], "add", "1"],
        ["-o", "o.mgz", s["ca"], "add", "1"],
        ["-o", "o.txt", s["ca"], "add", "1"],
        ["-o", "3.5", s["ca"], "add", "1"],
        ["-e", "-o", "o.txt", s["ca"], "add", "1"],
        ["-e", "-o", "o.mgz", s["ca"], "add", "1"],
        ["-e", "-o", "plain", s["ca"], "add", "1"],
        ["--strictExtensions", "-o", "3.5", s["ca"], "add", "1"],
        ["--output", "o2.crv", s["ca"], "add", "1"],
        ["-output", "o3.crv", s["ca"], "add", "1"],
        ["-O", "o4.crv", s["ca"], "add", "1"],
        ["-o", "same.crv", "-o", "o5.crv", s["ca"], "add", "1"],
        ["-o", "=vf", s["ca"], "add", "1"],
        ["-o", "=ca", "=ca", "div", "2"],
        ["=ca", "set", "3"],
        ["-o", "o6.crv", "=ca", "set", s["cb"]],
        # input types and sizes
        [s["ca"], "add", s["vf"]],
        [s["vf"], "add", s["ca"]],
        [s["ca"], "add", s["csmall"]],
        [s["czero"], "stats"],
        [s["czero"], "add", "1"],
        [s["ca"], "add", s["czero"]],
        [s["cneg"], "stats"],
        [s["cshort"], "stats"],
        ["-o", "sh.crv", s["cshort"], "add", "1"],
        [s["ca"], "add", s["cneg"]],
        [s["cold"], "stats"],
        [s["cext"], "stats"],
        [s["txt"], "stats"],
        [s["ca"], "add", s["txt"]],
        ["nofile.mgz", "stats"],
        [s["ca"], "add", "nofile"],
        [s["dir"], "stats"],
        ["-o", "o.crv", s["ca"], "mul"],
        ["-o", "o.crv", s["ca"], "set"],
        ["-o", "o.mgz", s["vf"], "masked"],
        [s["ca"], "norm", s["cb"]],
        # labels
        ["-l", s["lab"], "-o", "l1.crv", s["ca"], "add", "1"],
        ["--label", s["lab"], "-o", "l2.crv", s["ca"], "mul", s["cb"]],
        ["-label", s["lab"], s["cs"], "stats"],
        ["-l", s["lab"], "-o", "l3.txt", s["cs"], "ascii"],
        ["-l", s["lab"], "-o", "l4.crv", s["cs"], "norm"],
        ["-l", s["labempty"], "-o", "l5.crv", s["ca"], "add", "1"],
        ["-l", s["labempty"], s["ca"], "stats"],
        ["-l", s["labout"], "-o", "l6.crv", s["ca"], "add", "1"],
        ["-l", s["labneg"], "-o", "l7.crv", s["ca"], "add", "1"],
        ["-l", "nolabel.label", "-o", "l8.crv", s["ca"], "add", "1"],
        ["-l", s["labvol"], "-o", "l9.mgz", s["vf"], "mul", "2"],
        ["-l", s["lab"], "=ca", "set", "7"],
        # volumes
        ["-o", "v1.mgz", s["vf"], "mul", "1.50205015575164325309"],
        ["-o", "v2.mgz", s["vf"], "add", s["vf2"]],
        ["-o", "v3.mgz", s["vu"], "sub", s["vs"]],
        ["-o", "v4.mgz", s["vi"], "div", s["vf"]],
        ["-o", "v5.mgz", s["v3"], "mul", "2"],
        ["-o", "v6.mgz", s["vf"], "and", s["mask"]],
        ["-o", "v7.mgz", s["vf"], "masked", s["mask"]],
        ["-o", "v8.mgz", s["vf"], "add", s["vgeo"]],
        ["-o", "v9.mgz", s["vgeo"], "add", s["vf"]],
        ["-o", "v10.mgz", s["vf"], "add", s["vother"]],
        ["-o", "v11.mgz", s["vf"], "add", s["vsmall"]],
        ["-o", "v12.nii.gz", s["vf"], "sqrt"],
        ["-o", "v13.mgz", s["vnii"], "abs"],
        ["-o", "v14", s["vf"], "log"],
        ["-o", "v15.crv", s["vf"], "add", "1"],
        ["-e", "-o", "v16.nii", s["vf"], "add", "1"],
        ["-o", "v17.mgz", s["vf"], "norm"],
        ["-o", "v18.mgz", s["vs"], "andbw", s["vi"]],
        ["-o", "v19.mgz", s["vf"], "pow", "0.5"],
        ["-o", "v20.mgz", s["sv"], "and", s["sv"]],
        [s["vf"], "add", "1"],
        [s["vf"], "stats"],
        [s["v3"], "stats"],
        ["-o", "v.txt", s["vu"], "ascii"],
        ["-verbosity", "1", "-o", "v21.mgz", s["vf"], "mul", "2"],
        ["--verbosity", "1", "-o", "v22.crv", s["ca"], "add", s["cb"]],
        ["--verbosity", "2", s["cs"], "stats"],
        ["--verbosity", "-1", "-l", s["lab"], s["cs"], "max"],
        ["-o", "=vf", "=vf", "mul", "0.5"],
        # names past the program's buffers
        ["-o", LONG, s["ca"], "add", "1"],
        ["-l", LONG, s["ca"], "add", "1"],
        [LONG, "stats"],
        [s["ca"], "add", LONG],
        [NEAR, "stats"],
        ["-o", NEAR, s["vf"], "add", "1"],
        ["-e", "-o", "z" * 65532, s["ca"], "add", "1"],
    ]
    return c


def mask(text: str, port: bool, folder: Path) -> str:
    """The messages without what differs between the sides: the peak memory, the time, the
    case's folder (messages that name a file by its absolute name), the candidate's notices."""
    text = VMPEAK.sub(r"\1<kB>", STAMP.sub("TimeStamp: <time>", text))
    text = text.replace(str(folder), "<folder>")
    if port:
        text = "".join(x for x in text.splitlines(True) if not x.startswith("larmorx: "))
    return text


def status(code: int) -> int:
    """An exit status as a shell reports it: a process ended by signal ``n`` as ``128 + n``."""
    return 128 - code if code < 0 else code


def notices(text: str) -> list[str]:
    return sorted(
        {m.group(1) for m in re.finditer(r"^larmorx: upstream defect (\S+) ", text, re.M)}
    )


def written(folder: Path) -> dict[str, Path]:
    return {p.name: p for p in sorted(folder.rglob("*")) if p.is_file()}


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME / "bin" / "mris_calc"


def prepare(args: list[str], f: dict[str, Path], d: Path) -> list[str]:
    """The case's arguments in its folder ``d``: ``=x`` copies input ``x`` there."""
    out = []
    for a in args:
        if a.startswith("=") and a[1:] in f:
            src = f[a[1:]]
            if not (d / src.name).exists():
                shutil.copy(src, d / src.name)
            out.append(src.name)
        else:
            out.append(a)
    return out


def run(
    seed: int,
    candidate: list[str],
    *,
    modes: tuple[str, ...] = ("reproduce", "fix"),
    only: str | None = None,
    show: int = 6,
    skip: frozenset[str] = NOT_COMPARABLE,
    tag: str = "candidate",
) -> dict:
    """Every case of ``seed``: the official program against ``candidate`` in each of ``modes``
    (the first is the one that must give FreeSurfer's results); the counts and each case's row."""
    ws = workspace()
    exe = official()
    f = inputs(ws, seed)
    all_cases = cases(f)
    keep = {int(x) for x in only.split(",")} if only else set(range(len(all_cases)))
    work = ws / "oracles" / "inputs" / "fs-calc-prog" / f"work-{tag}-{seed}"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    rows = []
    verdicts: Counter = Counter()
    shown = 0
    for i, raw in enumerate(all_cases):
        if i not in keep:
            continue
        sides = {}
        for side in ("official", *modes):
            d = work / f"{i:03d}" / side
            if d.exists():
                shutil.rmtree(d)
            d.mkdir(parents=True)
            args = prepare(raw, f, d)
            copied = [d / f[a[1:]].name for a in raw if a.startswith("=") and a[1:] in f]
            if side == "official":
                done = oracle_cache.run(
                    ["mris_calc", *args],
                    env=env,
                    cwd=d,
                    outputs=[d],
                    policy=POLICY,
                    timeout=600,
                    runner="fs-calc-prog",
                    inputs=copied,
                )
                sides[side] = (done.returncode, done.stdout, done.stderr, d)
            else:
                p = subprocess.run(
                    [*(w.replace("{mode}", side) for w in candidate), *args],
                    env=env,
                    cwd=d,
                    capture_output=True,
                    text=True,
                    timeout=600,
                )
                sides[side] = (p.returncode, p.stdout, p.stderr, d)
        oc, oo, oe, od = sides["official"]
        row = {"case": i, "args": [a if len(a) < 200 else f"<{len(a)} bytes>" for a in raw],
               "official_exit": status(oc)}  # fmt: skip
        for side in modes:
            pc, po, pe, pd = sides[side]
            diffs = []
            if pc != status(oc):
                diffs.append(f"exit {pc} != {status(oc)}")
            if mask(po, True, pd) != mask(oo, False, od):
                diffs.append("stdout")
            if mask(pe, True, pd) != mask(oe, False, od):
                diffs.append("stderr")
            fo, fp = written(od), written(pd)
            if sorted(fo) != sorted(fp):
                diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
            for name in sorted(set(fo) & set(fp)):
                cmp = fsfiles.compare(fo[name], fp[name], ctx, name=name)
                if not cmp.same:
                    diffs.append(f"{name}: {cmp.verdict} {cmp.detail[:200]}")
            ids = notices(pe)
            row[side] = {"exit": pc, "differ": diffs, "notices": ids}
            rep_ids = notices(sides[modes[0]][2])
            if any(x in skip for x in rep_ids):
                row[side]["skip"] = True
                verdicts[f"{side}-skip"] += 1
                continue
            if side == modes[0]:
                verdict = "identical" if not diffs else "different"
            else:
                verdict = (
                    "fix-identical"
                    if not diffs
                    else ("fix-explained" if ids else "fix-unexplained")
                )
            verdicts[verdict] += 1
            if diffs and shown < show and (side == modes[0] or not ids):
                shown += 1
                print(f"case {i} {side}: {row['args']}\n  {diffs}\n  official stdout tail: "
                      f"{oo[-300:]!r}\n  candidate stdout tail: {po[-300:]!r}\n  official "
                      f"stderr: {oe[-300:]!r}\n  candidate stderr: {pe[-400:]!r}",
                      flush=True)  # fmt: skip
        rows.append(row)
        print(f"{i:3d} exit {oc} " + " ".join(
            f"{m} {'ok' if not row[m]['differ'] else 'DIFF ' + ','.join(row[m]['notices'])}"
            for m in modes), flush=True)  # fmt: skip
    return {"seed": seed, "cases": len(rows), "verdicts": dict(verdicts), "rows": rows}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--candidate", required=True, help="the command line; {mode} is replaced")
    ap.add_argument("--only", help="comma-separated case numbers")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=6)
    a = ap.parse_args(argv)
    summary = run(a.seed, a.candidate.split(), only=a.only, show=a.show)
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    v = summary["verdicts"]
    return 1 if v.get("different", 0) + v.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
