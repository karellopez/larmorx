# SPDX-License-Identifier: Apache-2.0
"""Head-motion correction: FSL's mcflirt against larmorx's clean-room original
(``larmorx mri hmc``, held to its record's thresholds) and its replica (``larmorx-nc fsl
mcflirt``, file by file and bit for bit).

A series of 4 to 8 volumes: a head-like phantom moved by a small random rigid motion in each
volume (rational rotations, evaluated exactly at the voxels), with noise; random options of
mcflirt's (cost, bins, degrees of freedom, reference volume or a separate reference, stages,
smoothing, final interpolation, ``-meanvol``, ``-stats``, the outputs). mcflirt runs live,
through the oracle cache, in its clean environment (``oracles/fsl-6.0.7/mcflirt/README.md``);
the original is then compared by the record's runner (``larmorx_validation.parity.mri_hmc``,
which reads a recorded run: the case writes one in a temporary oracle directory), the replica
by its suite's comparator (``larmorx_validation.parity.fsl_mcflirt_replica``).
"""

from __future__ import annotations

import functools
import json
import os
import shutil
import tempfile
from pathlib import Path

from larmorx_validation.difftest import gen, inputs
from larmorx_validation.difftest.core import Tool, arguments, group, required
from larmorx_validation.difftest.tools import _ants


def draw(d: gen.Draw) -> dict:
    n = [d.integer("nx", 12, 32), d.integer("ny", 12, 32), d.integer("nz", 6, 20)]
    vols = d.integer("volumes", 4, 8)
    spacing = d.choice("spacing", (2.0, 3.0, 3.5, 4.0))
    geometry = gen.plumb_geometry((spacing, spacing, d.choice("slice spacing", (spacing, 4.0, 3.0))), tuple(-(v - 1) * spacing / 2 for v in n))
    if d.chance("oblique", 0.2):
        geometry["quaternion"] = [d.integer("qa", 10, 30), d.integer("qb", -2, 2), 0, d.integer("qd", -2, 2)]
    content = {"type": "head", "seed": d.seed(), "centre": [0.0, 0.0, 0.0], "size": float(min(n[0], n[1]) * spacing * 0.42), "axes": [1.0, 1.1, 0.9], "bias": [0.0, 0.0, 0.0, 0.0], "noise": d.choice("noise", (0.0, 1.0, 3.0)), "tissues": d.choice("tissues", ("epi", "t2"))}
    motions = []
    for t in range(vols):
        if t == 0 and d.chance("first at rest", 0.5):
            motions.append({"quaternion": [1, 0, 0, 0], "translation": [0.0, 0.0, 0.0], "centre": [0.0, 0.0, 0.0]})
        else:
            motions.append(gen.draw_rigid(d.child(f"motion{t}"), degrees="small", shift=d.choice(f"shift{t}", (0.5, 1.0, 2.0))))
    series = {
        "shape": [*n, vols],
        "geometry": geometry,
        "content": content,
        "motion": motions,
        "values": {"scale": 1.0, "offset": 0.0},
        "dtype": d.choice("dtype", ("float32", "int16", "int16", "uint16")),
        "scaling": None,
        "timing": {"tr": 2.0, "tunits": "sec", "slice_code": 0},
        "suffix": ".nii.gz",
    }
    in_files = {"in": series}
    groups = [required("input", "-in", "{in:in}"), required("output", "-out", "{run}/out_mcf.nii.gz")]
    options = []
    if d.chance("reffile", 0.4):
        ref = {**series, "shape": n, "motion": [{"quaternion": [1, 0, 0, 0], "translation": [0.0, 0.0, 0.0], "centre": [0.0, 0.0, 0.0]}]}
        ref.pop("timing")
        in_files["ref"] = ref
        groups.append(group("reffile", "-reffile", "{in:ref}"))
        options.append("-reffile")
    elif d.chance("refvol", 0.3):
        groups.append(group("refvol", "-refvol", str(d.integer("refvol value", 0, vols - 1))))
        options.append("-refvol")
    for name, p, words in (
        ("cost", 0.4, lambda: ["-cost", d.choice("cost value", ("mutualinfo", "corratio", "normcorr", "normmi", "leastsquares"))]),
        ("bins", 0.15, lambda: ["-bins", d.choice("bins value", ("64", "128", "256"))]),
        ("dof", 0.25, lambda: ["-dof", d.choice("dof value", ("6", "7", "9", "12"))]),
        ("stages", 0.25, lambda: ["-stages", d.choice("stages value", ("1", "2", "3", "4"))]),
        ("smooth", 0.2, lambda: ["-smooth", d.choice("smooth value", ("0", "1", "2"))]),
        ("scaling", 0.1, lambda: ["-scaling", d.choice("scaling value", ("3", "6", "1"))]),
        ("rotation", 0.1, lambda: ["-rotation", d.choice("rotation value", ("0.5", "1", "2"))]),
        ("final", 0.25, lambda: [d.choice("final interpolation", ("-spline_final", "-sinc_final", "-nn_final"))]),
        ("meanvol", 0.15, lambda: ["-meanvol"]),
        ("stats", 0.15, lambda: ["-stats"]),
        ("fudge", 0.05, lambda: ["-fudge"]),
        ("gdt", 0.05, lambda: ["-gdt"]),
    ):
        if d.chance(name, p):
            w = words()
            groups.append(group(name, *w))
            options.append(w[0] if len(w) == 1 or name in ("final",) else f"{w[0]} {w[1]}")
    outs = [o for o in ("-mats", "-plots", "-rmsrel", "-rmsabs") if d.chance(o, 0.7)]
    if outs:
        groups.append(group("outputs", *outs))
        options += outs
    return {"groups": groups, "inputs": in_files, "options": options, "extra": {"min_size": 8, "min_volumes": 4, "keep_inputs": ["in"]}}


@functools.cache
def _fsl() -> Path:
    from larmorx_validation.parity.fsl_mcflirt_replica import fsldir

    return fsldir()


def case(recipe: dict, impl: str, *, case_prefix: str | None = None, cid: str | None = None, description: str | None = None, **kw):
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    payload = {"recipe": recipe, "prefix": prefix, "impl": impl}
    return Case(cid or _ants.case_id(recipe), "difftest", description or _ants.describe(recipe), payload)


def _args_for(recipe: dict, prefix: str, run_dir: Path, copy_inputs: bool) -> list[str]:
    """mcflirt's arguments: inputs copied into the run directory (as the recorded runs keep
    them), outputs named in it."""
    names = {}
    for name in recipe["inputs"]:
        src = inputs.path(inputs.key(prefix, name))
        target = run_dir / f"{name}.nii.gz"
        if copy_inputs and not target.exists():
            shutil.copy(src, target)
        names[name] = str(target)
    words = arguments(recipe)
    out = []
    for w in words:
        for name, p in names.items():
            w = w.replace("{in:" + name + "}", p)
        out.append(w.replace("{run}", str(run_dir)))
    return out


def _run_original(case, checks) -> None:
    """mcflirt runs live into a recorded run of a temporary oracle directory, which the
    record's runner (``parity.mri_hmc.run_case``) replays larmorx against."""
    from larmorx_validation.parity import fsl_mcflirt_replica as rep
    from larmorx_validation.parity import mri_hmc
    from larmorx_validation.parity.harness import Case

    p = case.payload
    name = "difftest/case"
    with tempfile.TemporaryDirectory(prefix="lx-dt-hmc-") as t:
        root = Path(t) / "oracles" / "fsl-6.0.7" / "mcflirt"
        run_dir = root / "runs" / name
        run_dir.mkdir(parents=True)
        args = _args_for(p["recipe"], p["prefix"], run_dir, copy_inputs=True)
        settings: dict[str, str | None] = {}
        done = rep.run_oracle(args, cwd=run_dir, settings=settings)
        (run_dir / "stdout0.txt").write_text(done.stdout, encoding="utf-8")
        (run_dir / "stderr0.txt").write_text(done.stderr, encoding="utf-8")
        env = rep._environment(settings)
        run = {
            "name": name,
            "notes": "a random case of the differential tests",
            "env": env,
            "inputs": {k: {"path": str(run_dir / f"{k}.nii.gz")} for k in p["recipe"]["inputs"]},
            "steps": [{"argv": [str(_fsl() / "bin" / "mcflirt"), *args], "exit": done.code}],
        }
        (run_dir / "run.json").write_text(json.dumps(run, indent=1), encoding="utf-8")
        (root / "manifest.json").write_text(json.dumps({"runs": {name: {"ok": done.code == 0}}}), encoding="utf-8")
        saved = os.environ.get("LARMORX_MCFLIRT_ORACLE")
        os.environ["LARMORX_MCFLIRT_ORACLE"] = str(root)
        mri_hmc.oracle_dir.cache_clear()
        mri_hmc.manifest.cache_clear()
        try:
            mri_hmc.run_case(Case(name, "difftest", "", name), checks)
        finally:
            if saved is None:
                os.environ.pop("LARMORX_MCFLIRT_ORACLE", None)
            else:
                os.environ["LARMORX_MCFLIRT_ORACLE"] = saved
            mri_hmc.oracle_dir.cache_clear()
            mri_hmc.manifest.cache_clear()


def _run_replica(case, checks) -> None:
    """mcflirt and the replica in directories of their own, compared file by file
    (``parity.fsl_mcflirt_replica.compare``), then the replica's fix pass."""
    from larmorx_validation.parity import defects
    from larmorx_validation.parity import fsl_mcflirt_replica as rep

    p = case.payload
    with tempfile.TemporaryDirectory(prefix="lx-dt-mcf-") as a_, tempfile.TemporaryDirectory(prefix="lx-dt-mcf-") as b_:
        a, b = Path(a_), Path(b_)

        def prepare(d: Path) -> list[str]:
            return _args_for(p["recipe"], p["prefix"], d, copy_inputs=True)

        settings: dict[str, str | None] = {}
        oracle = rep.run_oracle(prepare(a), cwd=a, settings=settings)
        replica = rep.run_replica(prepare(b), cwd=b, settings=settings, mode=defects.REPRODUCE, threads=1)
        exclude = {f"{n}.nii.gz" for n in p["recipe"]["inputs"]}
        rep.compare(
            checks,
            rep.Completed(oracle.code, oracle.stdout.replace(str(a), "<run>"), oracle.stderr.replace(str(a), "<run>")),
            rep.Completed(replica.code, replica.stdout.replace(str(b), "<run>"), replica.stderr.replace(str(b), "<run>")),
            a,
            b,
            exclude,
        )
        if replica.code == 0:
            rep.fix_pass(checks, prepare, settings, b, replica, exclude)


def _unavailable_mcflirt() -> str:
    return "" if (_fsl() / "bin" / "mcflirt").is_file() else "FSL's mcflirt was not found"


def _unavailable_replica() -> str:
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_binary

    why = _unavailable_mcflirt()
    return why or ("" if replica_binary() is not None else "the larmorx-nc program was not found")


ORIGINAL = Tool(
    name="mri-hmc",
    title="Head-motion correction (clean-room original)",
    suite="mri-hmc",
    implementations=("original",),
    draw=draw,
    case=case,
    run=lambda impl: _run_original,
    exact=lambda impl: False,
    timeout=300,
    weight=1.0,
    unavailable=_unavailable_mcflirt,
    family="fsl",
    payload_fields=(),
)
REPLICA = Tool(
    name="fsl-mcflirt",
    title="mcflirt (replica)",
    suite="fsl-mcflirt-replica",
    implementations=("replica",),
    draw=draw,
    case=case,
    run=lambda impl: _run_replica,
    # Bit-identity is the suite's own pass criterion ("every file bit-identical").
    exact_checks=(),
    timeout=300,
    weight=1.0,
    unavailable=_unavailable_replica,
    family="fsl",
    payload_fields=(),
)
