# SPDX-License-Identifier: Apache-2.0
"""mri_normalize as a black box: FreeSurfer 8.2.0's official program against a candidate, case by case.

An open probe (``README.md``): it writes seeded synthetic inputs (MGZ volumes of a head-like pattern
in uchar, short and float, 32^3, 64^3, a non-cubic one and two conformed 256^3 ones, a segmentation
with the white matter's labels 2 and 41, a brain mask, control and bias volumes, a control-point
file, a label, two closed surfaces) into ``<workspace>/oracles/inputs/fs-normalize-prog/<seed>/``,
runs each case's command line through the official program (the unit oracle's
``bin/mri_normalize``, found on the ``PATH`` by its name as recon-all runs it) by the
oracle-output cache at one thread, and through the candidate (``--candidate``, a command line whose
``{mode}`` becomes ``reproduce`` and then ``fix``), each in a folder of its own (the program writes
relative files). Compared: the exit status, standard output and error (the timing line's numbers,
the ``VmPeak`` figure and the candidate's ``larmorx:`` notices aside) and every file written
(FreeSurfer's provenance aside: ``larmorx_validation.freesurfer.files``). ``reproduce`` must give
FreeSurfer's on every case but those whose reproduce run notes a defect of ``NOT_COMPARABLE`` (or
``PE``), which are counted; ``fix`` may differ only with a notice.

    python -P validation/investigations/freesurfer-mri-normalize-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_normalize'
"""

from __future__ import annotations

import argparse
import json
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

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer import files as fsfiles  # noqa: E402
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

#: The defects (``larmorx_core::defects``) whose cases are counted, not compared: FreeSurfer reads
#: memory it does not own, crashes or never ends there, or seeds its random numbers from the clock
#: (FS-04, FS-53, FS-58, FS-540, FS-541, FS-546, FS-549, FS-588, FS-589, FS-591, FS-593, FS-597,
#: FS-598, FS-600, FS-601, FS-1014, FS-1015, FS-1016, FS-1017).
NOT_COMPARABLE = frozenset(
    {"FS-04", "FS-53", "FS-58", "FS-540", "FS-541", "FS-546", "FS-549", "FS-588", "FS-589",
     "FS-591", "FS-593", "FS-597", "FS-598", "FS-600", "FS-601", "FS-1014", "FS-1015", "FS-1016",
     "FS-1017"}
)  # fmt: skip
#: The number in the message on rejected control points is not the program's result (FS-596).
REJECTED = re.compile(r"!!!!! -?[0-9]+ control points rejected")
PERROR = re.compile(r"^No such file or directory\nerror: Invalid argument$", re.M)
#: FreeSurfer's runs compared with their entries: the timing line and the memory figure vary, and
#: its files' provenance (the command line with its time).
POLICY = oracle_cache.Policy(
    threads=1,
    family="freesurfer",
    formats="freesurfer",
    volatile=(r"3D bias adjustment took \d+ minutes and \d+ seconds\.", r"VmPeak +-?\d+"),
)
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


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / "oracles" / "freesurfer-8.2.0-unit" / "bin" / "mri_normalize"


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
    work = ws / "oracles" / "inputs" / "fs-normalize-prog" / f"work-{tag}-{seed}"
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
        for side in ("official", *modes):
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
                    policy=POLICY,
                    timeout=1800,
                    runner="fs-normalize-prog",
                )
                sides[side] = (done.returncode, done.stdout, done.stderr, d)
            else:
                p = subprocess.run(
                    [*(w.replace("{mode}", side) for w in candidate), *args],
                    env=env,
                    cwd=d,
                    capture_output=True,
                    text=True,
                    timeout=1800,
                )
                sides[side] = (p.returncode, p.stdout, p.stderr, d)
        oc, oo, oe, od = sides["official"]
        row = {"case": i, "args": args, "official_exit": oc}
        for side in modes:
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
            rep_ids = notices(sides[modes[0]][2])
            # PE: after its first message of a failed open, FreeSurfer prints the error
            # "Invalid argument" where the open's error ("No such file or directory") is expected,
            # when its standard error is opened write-only (a pipe, a file): counted, not compared
            if PERROR.search(oe) and diffs == ["stderr"]:
                rep_ids = [*rep_ids, "PE"]
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
                print(f"case {i} {side}: {args}\n  {diffs}\n  official stdout tail: "
                      f"{oo[-300:]!r}\n  candidate stdout tail: {po[-300:]!r}\n  candidate stderr: "
                      f"{pe[-400:]!r}", flush=True)  # fmt: skip
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
