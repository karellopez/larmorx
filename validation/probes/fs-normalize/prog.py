# SPDX-License-Identifier: Apache-2.0
"""mri_normalize as a whole program: FreeSurfer 8.2.0's official binary against the port, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (its cases name the program's options and the
volumes its paths read; running the official program is a black-box run).

Writes seeded synthetic inputs (MGZ volumes of a head-like pattern in uchar, short and float, a
conformed 256^3 one, a segmentation with the white matter's labels, a brain mask, control and
bias volumes, a control-point file, a label, two surfaces) into
``<workspace>/oracles/inputs/fs-normalize-prog/<seed>/``, and runs each case's command line through
the unit oracle's ``bin/mri_normalize`` (byte-identical to the official program) by the
oracle-output cache at one thread, and through the port (``larmorx-freesurfer freesurfer
mri_normalize``) with ``--upstream-defects reproduce`` and ``fix``, each in a folder of its own
(the program writes relative files: ``m.mgz``, ``nc.mgz``, ``r.mgz``, ...). Compared: the exit
status, standard output and error (the timing line's numbers, the ``VmPeak`` figure and the
port's ``larmorx:`` notices aside) and every file written (FreeSurfer's provenance aside:
``larmorx_validation.freesurfer.files``). ``reproduce`` must give FreeSurfer's on every case;
``fix`` may differ only with a notice.

    python -P validation/probes/fs-normalize/prog.py --seed 1 [--only 3,7] [--json out.json]
    python -P validation/probes/fs-normalize/prog.py --golden

``--golden``: the official program on the golden cases' inputs (written by the port's example
``fs_normalize_golden``), hashed as ``tests/mri_normalize_golden.rs`` hashes the port's run.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer import files as fsfiles  # noqa: E402
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

_spec = importlib.util.spec_from_file_location(
    "check_filter", HERE.parent / "fs-filter" / "check.py"
)
check_filter = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(check_filter)

# the cases' reproduce notices that make a case not comparable (counted, not compared): the fs-filter
# and fs-normalize probes' SKIP (FreeSurfer reads memory it does not own, crashes, or seeds from the
# clock) and FS-1017 (a step that gives no volume: FreeSurfer crashes)
SKIP = frozenset(check_filter.SKIP) | frozenset(check.SKIP) | {"FS-1017"}
REJECTED = re.compile(r"!!!!! -?[0-9]+ control points rejected")
PERROR = re.compile(r"^No such file or directory\nerror: Invalid argument$", re.M)
TIMING = re.compile(r"3D bias adjustment took \d+ minutes and \d+ seconds\.")
VMPEAK = re.compile(r"(#VMPC# mri_normalize VmPeak +)-?\d+")

# FreeSurfer's conformed orientation (LIA, 1 mm, centred)
LIA = np.array([[-1.0, 0, 0, 128], [0, 0, 1, -128], [0, -1, 0, 128], [0, 0, 0, 1]])


def head(rng: np.random.Generator, shape: tuple[int, int, int], scale: float) -> np.ndarray:
    """A head-like pattern: white matter near 110 in the middle, grey matter, CSF, a dark rim."""
    idx = np.indices(shape).astype(np.float64)
    c = [(s - 1) / 2 for s in shape]
    r = np.sqrt(sum(((idx[i] - c[i]) / (shape[i] / 2)) ** 2 for i in range(3)))
    v = np.where(r < 0.45, 110.0, np.where(r < 0.6, 75.0, np.where(r < 0.75, 35.0, 4.0)))
    v = v * (1.0 + 0.15 * (idx[0] / shape[0] - 0.5))  # a smooth bias along x
    v = v + rng.normal(0.0, 5.0, shape)
    return np.clip(v * scale, 0, None)


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def inputs(ws: Path, seed: int) -> dict[str, Path]:
    d = ws / "oracles" / "inputs" / "fs-normalize-prog" / str(seed)
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(seed)
    aff = np.diag([1.0, 1.0, 1.0, 1.0])
    aff[:3, 3] = [-16.0, -16.0, -16.0]
    f: dict[str, Path] = {}
    t = head(rng, (32, 32, 32), 1.0)
    f["u8"] = save(d / "t1_u8_32.mgz", np.clip(np.rint(t), 0, 255).astype(np.uint8), aff)
    f["f32"] = save(d / "t1_f32_32.mgz", t.astype(np.float32), aff)
    f["s16"] = save(d / "t1_s16_32.mgz", np.rint(t * 4).astype(np.int16), aff)
    aff2 = np.diag([1.2, 0.9, 1.1, 1.0])
    aff2[:3, 3] = [-20.0, -15.0, -14.0]
    t2 = head(rng, (40, 36, 28), 1.0)
    f["u8b"] = save(d / "t1_u8_40x36x28.mgz", np.clip(np.rint(t2), 0, 255).astype(np.uint8), aff2)
    aff64 = np.diag([1.0, 1.0, 1.0, 1.0])
    aff64[:3, 3] = [-32.0, -32.0, -32.0]
    t64 = head(rng, (64, 64, 64), 1.0)
    f["u864"] = save(d / "t1_u8_64.mgz", np.clip(np.rint(t64), 0, 255).astype(np.uint8), aff64)
    f["f3264"] = save(d / "t1_f32_64.mgz", (t64 * 0.8).astype(np.float32), aff64)
    big = head(rng, (256, 256, 256), 1.0)
    f["conf"] = save(d / "t1_u8_256.mgz", np.clip(np.rint(big), 0, 255).astype(np.uint8), LIA)
    f["conff"] = save(d / "t1_f32_256.mgz", (big * 1.4).astype(np.float32), LIA)
    idx = np.indices((32, 32, 32)).astype(np.float64)
    r = np.sqrt(sum(((idx[i] - 15.5) / 16) ** 2 for i in range(3)))
    aseg = np.where(r < 0.45, np.where(idx[0] < 16, 2, 41), np.where(r < 0.6, 3, 0)).astype(
        np.int32
    )
    f["aseg"] = save(d / "aseg_32.mgz", aseg, aff)
    f["asegu8"] = save(d / "aseg_u8_32.mgz", aseg.astype(np.uint8), aff)
    f["mask"] = save(d / "mask_32.mgz", np.where(r < 0.7, 1, 0).astype(np.uint8), aff)
    ctrl = np.where((r < 0.4) & (rng.random((32, 32, 32)) < 0.2), 1, 0).astype(np.uint8)
    f["ctrl"] = save(d / "ctrl_32.mgz", ctrl, aff)
    f["ctrlf"] = save(d / "ctrl_f32_32.mgz", ctrl.astype(np.float32), aff)
    bias = (1.0 + 0.1 * (idx[1] / 32 - 0.5)).astype(np.float32)
    f["bias"] = save(d / "bias_32.mgz", bias, aff)
    ren = np.clip(np.rint(t), 0, 255).astype(np.uint8)
    ren[(r < 0.4) & (rng.random((32, 32, 32)) < 0.3)] = 110
    f["renorm"] = save(d / "renorm_32.mgz", ren, aff)
    f["renormb"] = save(
        d / "renorm_u8_16.mgz", ren[::2, ::2, ::2].copy(), np.diag([2.0, 2.0, 2.0, 1])
    )
    # a control-point file (scanner coordinates) and a label (voxel centres' coordinates)
    pts = [(-2.0, 1.0, 0.5), (3.0, -4.0, 2.0), (0.0, 0.0, 0.0), (5.0, 5.0, -5.0)]
    lines = [f"{x} {y} {z}" for x, y, z in pts] + ["info", f"numpoints {len(pts)}", "useRealRAS 1"]
    (d / "control.dat").write_text("\n".join(lines) + "\n", encoding="utf-8")
    f["cp"] = d / "control.dat"
    lab = [f"{i} {x:.3f} {y:.3f} {z:.3f} 0.000000" for i, (x, y, z) in enumerate(pts)]
    (d / "wm.label").write_text(
        "#!ascii label  , from subject  vox2ras=TkReg\n" + f"{len(pts)}\n" + "\n".join(lab) + "\n",
        encoding="utf-8",
    )
    f["label"] = d / "wm.label"
    # two closed surfaces (octahedra) inside the head, in tkregister coordinates
    v = np.array([[6, 0, 0], [-6, 0, 0], [0, 6, 0], [0, -6, 0], [0, 0, 6], [0, 0, -6]], float)
    fa = np.array(
        [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]]
    )
    nib.freesurfer.write_geometry(str(d / "lh.white"), v, fa)
    nib.freesurfer.write_geometry(str(d / "rh.white"), v * 0.7 + [1.0, 0, 0], fa)
    f["lh"], f["rh"] = d / "lh.white", d / "rh.white"
    return f


def cases(f: dict[str, Path]) -> list[list[str]]:
    """The command lines (``@x`` an input's absolute name; outputs are relative)."""
    s = ["-seed", "1234"]
    c: list[list[str]] = [
        [],
        ["@u8"],
        ["-u"],
        ["--help"],
        ["--version"],
        ["-xyz", "@u8", "out.mgz"],
        ["@u8", "out.mgz"],
        [*s, "@u8", "out.mgz"],
        [*s, "-mprage", "@u8", "out.mgz"],
        ["-g", "1", *s, "-mprage", "@u8", "out.mgz"],
        [*s, "@f32", "out.mgz"],
        [*s, "@s16", "out.mgz"],
        [*s, "@u8b", "out.mgz"],
        [*s, "-n", "1", "@u8", "out.mgz"],
        [*s, "-n", "0", "@u8", "out.mgz"],
        [*s, "-no1d", "@u8", "out.mgz"],
        [*s, "-no1d", "-snr", "@u8", "out.mgz"],
        [*s, "-gentle", "@u8", "out.mgz"],
        [*s, "-monkey", "@u8", "out.mgz"],
        [*s, "-a", "15", "-b", "5", "@u8", "out.mgz"],
        [*s, "-sigma", "4", "@u8", "out.mgz"],
        [*s, "-prune", "1", "@u8", "out.mgz"],
        [*s, "-washu_mprage", "@u8", "out.mgz"],
        [*s, "-T2", "@u8", "out.mgz"],
        [*s, "-noskull", "@u8", "out.mgz"],
        [*s, "-mask", "@mask", "@u8", "out.mgz"],
        [*s, "-aseg", "@aseg", "@u8", "out.mgz"],
        [*s, "-mprage", "-aseg", "@aseg", "-mask", "@mask", "@u8", "out.mgz"],
        [*s, "-aseg", "@asegu8", "-c", "cp.mgz", "@f32", "out.mgz"],
        [*s, "-aseg", "@aseg", "@u8b", "out.mgz"],
        [*s, "-f", "@cp", "@u8", "out.mgz"],
        [*s, "-fonly", "@cp", "@u8", "out.mgz"],
        [*s, "-f", "@cp", "-no-gentle-cp", "@u8", "out.mgz"],
        [*s, "-label", "@label", "@u8", "out.mgz"],
        [*s, "-lonly", "@label", "@u8", "out.mgz"],
        [*s, "-w", "ctrl.mgz", "bias.mgz", "@u8", "out.mgz"],
        [*s, "-grad", "8", "@u8", "out.mgz"],
        [*s, "-r", "@ctrl", "@bias", "@u8", "out.mgz"],
        [*s, "-r", "@ctrlf", "@bias", "@f32", "out.mgz"],
        [*s, "-l", "@ctrl", "@bias", "@u8", "out.mgz"],
        [*s, "-renorm", "@renorm", "@u8", "out.mgz"],
        [*s, "-renorm", "@renorm", "-erode", "1", "@u8", "out.mgz"],
        [*s, "-renorm", "@renormb", "@u8", "out.mgz"],
        [*s, "-surface", "@lh", "identity.nofile", "@u8", "out.mgz"],
        [
            *s,
            "-surface",
            "@lh",
            "identity.nofile",
            "-surface",
            "@rh",
            "identity.nofile",
            "@u8",
            "out.mgz",
        ],
        [
            *s,
            "-surface",
            "@lh",
            "identity.nofile",
            "-aseg",
            "@asegu8",
            "-nonmax_suppress",
            "0",
            "@u8",
            "out.mgz",
        ],
        [
            *s,
            "-surface",
            "@lh",
            "identity.nofile",
            "-erode",
            "1",
            "-nonmax_suppress",
            "0",
            "-w",
            "ctrl.mgz",
            "bias.mgz",
            "@u8",
            "out.mgz",
        ],
        [
            *s,
            "-surface",
            "@lh",
            "identity.nofile",
            "-checknorm",
            "@u8",
            "90",
            "120",
            "@u8",
            "out.mgz",
        ],
        [*s, "-surface", "@lh", "identity.nofile", "-mask_sigma", "1", "5", "@u8", "out.mgz"],
        [*s, "-surface", "@lh", "identity.nofile", "-mask_orig", "@u8", "100", "@u8", "out.mgz"],
        [*s, "-surface", "@lh", "identity.nofile", "-T2", "@u8", "out.mgz"],
        [*s, "-surface", "@lh", "identity.nofile", "-label", "@label", "@u8", "out.mgz"],
        [*s, "-no1d", "-interior", "@lh", "@rh", "@u8", "out.mgz"],
        [*s, "-aseg", "@aseg", "-interior", "@lh", "@rh", "@u8", "out.mgz"],
        [*s, "-conform", "@u8b", "out.mgz"],
        [*s, "-noconform", "@u8", "out.mgz"],
        [*s, "@conf", "out.mgz"],
        ["-g", "1", *s, "-mprage", "@conf", "out.mgz"],
        [*s, "@conff", "out.mgz"],
        [*s, "@u864", "out.mgz"],
        ["-g", "1", *s, "-mprage", "@u864", "out.mgz"],
        [*s, "@f3264", "out.mgz"],
        [*s, "-aseg", "@aseg", "@u864", "out.mgz"],
        [*s, "-no1d", "@u864", "out.mgz"],
        [*s, "-no1d", "-snr", "@u864", "out.mgz"],
        [*s, "-gentle", "@u864", "out.mgz"],
        [*s, "-n", "1", "-prune", "1", "@u864", "out.mgz"],
        [*s, "-w", "ctrl.mgz", "bias.mgz", "@u864", "out.mgz"],
        [*s, "-grad", "8", "@u864", "out.mgz"],
        [*s, "-f", "@cp", "@u864", "out.mgz"],
        [*s, "-sigma", "2", "-a", "20", "-b", "6", "@u864", "out.mgz"],
        [*s, "/nonexistent/in.mgz", "out.mgz"],
        [*s, "@u8", "/nonexistent/dir/out.mgz"],
        [*s, "-mask", "/nonexistent/m.mgz", "@u8", "out.mgz"],
        [*s, "-aseg", "/nonexistent/a.mgz", "@u8", "out.mgz"],
        [*s, "-renorm", "/nonexistent/r.mgz", "@u8", "out.mgz"],
        [*s, "-r", "/nonexistent/c.mgz", "@bias", "@u8", "out.mgz"],
        [*s, "-surface", "/nonexistent/lh.white", "identity.nofile", "@u8", "out.mgz"],
        [*s, "-label", "/nonexistent/x.label", "@u8", "out.mgz"],
        [*s, "-aseg", "@aseg", "-conform", "@u8", "out.mgz"],
        ["-n"],
        [*s, "-a"],
    ]
    return [[str(f[a[1:]]) if a.startswith("@") else a for a in x] for x in c]


def mask(text: str, port: bool) -> str:
    text = TIMING.sub("3D bias adjustment took M minutes and S seconds.", text)
    text = REJECTED.sub("!!!!! ? control points rejected", text)
    text = VMPEAK.sub(r"\1N", text)
    if port:
        text = "".join(x for x in text.splitlines(True) if not x.startswith("larmorx: "))
    return text


def notices(text: str) -> list[str]:
    return sorted(
        {m.group(1) for m in re.finditer(r"^larmorx: upstream defect (\S+) ", text, re.M)}
    )


def written(folder: Path) -> dict[str, Path]:
    return {p.name: p for p in sorted(folder.iterdir()) if p.is_file()}


def port_binary(jobs: int) -> Path:
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(jobs),
         "--manifest-path", str(REPO / "crates-freesurfer" / "Cargo.toml")],
        check=True,
        env=env,
    )  # fmt: skip
    return REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"


def golden_line(line: str) -> str:
    """``mask_line`` of the golden test (``tests/mri_normalize_golden/common.rs``): the timing
    line's numbers, and a tissue level read from a histogram bin below 0 (FS-597: memory contents
    in FreeSurfer) as ``?`` without its padding."""
    if line.startswith("3D bias adjustment took "):
        return "3D bias adjustment took M minutes and S seconds."
    w = line.split(" ")
    keep: list[str | None] = list(w)
    for i in range(1, len(w)):
        if w[i] != "at" or w[i - 1] not in ("peak", "valley"):
            continue
        j = next((j for j in range(i + 1, len(w)) if w[j]), None)
        if j is None:
            continue
        if j + 1 < len(w) and w[j + 1].startswith("(-"):
            for k in range(i + 1, j):
                keep[k] = None
            keep[j] = "?"
    return " ".join(x for x in keep if x is not None)


def golden(ws: Path, env: dict[str, str], jobs: int) -> int:
    """The golden cases (``tests/mri_normalize_golden.rs``): the official program on the port's
    32^3 inputs, hashed as the test hashes the port's run; prints the value to record."""
    import hashlib

    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_normalize_golden", "-j", str(jobs), "--manifest-path",
         str(REPO / "crates-freesurfer" / "Cargo.toml")],
        check=True,
        env={k: v for k, v in os.environ.items() if k != "FSLDIR"},
    )  # fmt: skip
    ex = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_normalize_golden"
    root = ws / "oracles" / "inputs" / "fs-normalize-prog" / "golden"
    if root.exists():
        shutil.rmtree(root)
    (root / "in").mkdir(parents=True)
    subprocess.run([ex, "inputs", root / "in"], check=True)
    lines = subprocess.run([ex, "cases"], check=True, capture_output=True, text=True).stdout
    h = hashlib.sha256()
    for line in lines.splitlines():
        k, args, outs = line.split("\t")
        d = root / f"case-{k}"
        shutil.copytree(root / "in", d)
        done = oracle_cache.run(
            ["mri_normalize", *args.split()],
            env=env,
            cwd=d,
            outputs=[d],
            policy=oracle_cache.Policy(threads=1, family="freesurfer"),
            timeout=600,
            runner="fs-normalize-prog-golden",
        )
        text = "\n".join(golden_line(x) for x in done.stdout.splitlines())
        h.update(f"case {k} {done.returncode & 0xFF}\n{text}\n".encode())
        for name in outs.split():
            dig = subprocess.run(
                [ex, "digest", d / name], check=True, capture_output=True, text=True
            )
            h.update(f"{name} {dig.stdout.strip()}\n".encode())
        print(f"case {k}: exit {done.returncode}, {outs}", flush=True)
    print(f"golden OFFICIAL_SHA256 {h.hexdigest()}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--only", help="comma-separated case numbers")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=6)
    ap.add_argument("--golden", action="store_true", help="the golden cases' official value")
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / "oracles" / "freesurfer-8.2.0-unit" / "bin" / "mri_normalize"
    if a.golden:
        genv = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
                "OMP_NUM_THREADS": "1", "USER": "probe"}  # fmt: skip
        genv.update(freesurfer_env())
        return golden(ws, genv, a.jobs)
    port = port_binary(a.jobs)
    f = inputs(ws, a.seed)
    all_cases = cases(f)
    keep = {int(x) for x in a.only.split(",")} if a.only else set(range(len(all_cases)))
    work = ws / "oracles" / "inputs" / "fs-normalize-prog" / f"work-{a.seed}"
    # argv[0] as recon-all gives it (the program's messages print it): the name, found on the PATH
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    rows = []
    verdicts: Counter = Counter()
    shown = 0
    for i, args in enumerate(all_cases):
        if i not in keep:
            continue
        sides = {}
        for side in ("official", "reproduce", "fix"):
            d = work / f"{i:03d}" / side
            if d.exists():
                shutil.rmtree(d)
            d.mkdir(parents=True)
            if side == "official":
                done = oracle_cache.run(
                    ["mri_normalize", *args],
                    env=env,
                    cwd=d,
                    outputs=[d],
                    policy=oracle_cache.Policy(threads=1, family="freesurfer"),
                    timeout=1800,
                    runner="fs-normalize-prog",
                )
                sides[side] = (done.returncode, done.stdout, done.stderr, d)
            else:
                p = subprocess.run(
                    [port, "--upstream-defects", side, "freesurfer", "mri_normalize", *args],
                    env=env,
                    cwd=d,
                    capture_output=True,
                    text=True,
                    timeout=1800,
                )
                sides[side] = (p.returncode, p.stdout, p.stderr, d)
        oc, oo, oe, od = sides["official"]
        row = {"case": i, "args": args, "official_exit": oc}
        for side in ("reproduce", "fix"):
            pc, po, pe, pd = sides[side]
            diffs = []
            if pc != oc:
                diffs.append(f"exit {pc} != {oc}")
            if mask(po, True) != mask(oo, False):
                diffs.append("stdout")
            if mask(pe, True) != mask(oe, False):
                diffs.append("stderr")
            fo, fp = written(od), written(pd)
            if sorted(fo) != sorted(fp):
                diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
            for name in sorted(set(fo) & set(fp)):
                c = fsfiles.compare(fo[name], fp[name], ctx, name=name)
                if not c.same:
                    diffs.append(f"{name}: {c.verdict} {c.detail[:200]}")
            ids = notices(pe)
            row[side] = {"exit": pc, "differ": diffs, "notices": ids}
            rep_ids = notices(sides["reproduce"][2])
            # PE: the C library's first perror on a standard error opened write-only sets errno
            # to EINVAL, which FreeSurfer then prints; fs.io's LabelRead keeps the open's errno
            # (track A's item): such a case is counted, not compared
            if PERROR.search(oe) and diffs == ["stderr"]:
                rep_ids = [*rep_ids, "PE"]
            if any(x in SKIP for x in rep_ids):
                row[side]["skip"] = True
                verdicts[f"{side}-skip"] += 1
                continue
            if side == "reproduce":
                verdict = "identical" if not diffs else "different"
            else:
                verdict = (
                    "fix-identical"
                    if not diffs
                    else ("fix-explained" if ids else "fix-unexplained")
                )
            verdicts[verdict] += 1
            if diffs and shown < a.show and (side == "reproduce" or not ids):
                shown += 1
                print(f"case {i} {side}: {args}\n  {diffs}\n  official stdout tail: "
                      f"{oo[-300:]!r}\n  port stdout tail: {po[-300:]!r}\n  port stderr: "
                      f"{pe[-400:]!r}", flush=True)  # fmt: skip
        rows.append(row)
        print(f"{i:3d} exit {oc} reproduce {'ok' if not row['reproduce']['differ'] else 'DIFF'} "
              f"fix {'ok' if not row['fix']['differ'] else 'diff ' + ','.join(row['fix']['notices'])}",
              flush=True)  # fmt: skip
    summary = {"seed": a.seed, "cases": len(rows), "verdicts": dict(verdicts), "rows": rows}
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    bad = verdicts["different"] + verdicts["fix-unexplained"]
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
