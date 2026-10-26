# SPDX-License-Identifier: Apache-2.0
"""The data panel's worked examples of the ANTs tools (``docs/figures.md``, the data-panel case).

Each ANTs page gains, beside its existing examples (never replacing one), its panel cases: two
or three key datasets as whole volumes (``view="volume"``) and the others as three
full-resolution slices (``view="slices"``). An example is one call of an ANTs program on a
panel image (or one of fMRIPrep's own calls on a panel subject), run into a figures-cache entry
by:

- ANTs 2.6.5 (the repository's oracle, GCC 14.3) at one thread as built, the unmodified
  program (the headline);
- the same ANTs with CORE-MATH's correctly rounded functions preloaded (the explanation where
  glibc's maths is not correctly rounded);
- the same ANTs at more threads (its own variability, where it has any);
- larmorx in reproduce mode at several threads and at one (its result does not depend on
  them), and in its default mode (the D13 corrections).

Every comparison's metrics are computed over every voxel of the files; its verdict comes from
them (:func:`verdicts`), never typed. A program whose outputs are transforms (antsAI,
antsAffineInitializer) is shown through its transform: the moving image resampled onto the
fixed grid by the ANTs oracle's antsApplyTransforms (linear), one such image per run, and a
table of every run's parameters. The shown region is the whole field of view (only margins
where every array is exactly 0 are left out) for images whose defacing was checked
(larmorx-testdata ``panel.toml``, ``show = "head"``; every image this module shows is), else
the brain.
"""

from __future__ import annotations

import copy
import re
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache

#: The runs of an example: (id, role, label, sub); ``threads`` fills the larmorx and band rows.
LX_THREADS = 8


@dataclass(frozen=True)
class Spec:
    """One panel example of an ANTs program.

    ``args`` are the program's arguments: ``{in:<catalog path>}`` a larmorx-testdata file, an
    absolute path a file of an oracle run, ``{out:<name>}`` an output written into the run's
    directory. ``outputs`` lists ``(id, file name, label, unit)`` of the images shown (for a
    transform program, ``place`` names the moving and fixed images: each run's transform
    ``place[2]`` places the moving image on the fixed grid). ``grid`` is the image whose grid
    the outputs share (the shown input), ``inputs`` the input panels ``(id, path, label)``,
    each on that grid. ``labels`` are outputs drawn over the first input (masks, labels)."""

    tool: str
    case: str
    id: str
    title: str
    summary: str
    program: str
    args: tuple[str, ...]
    outputs: tuple[tuple[str, str, str, str], ...]
    grid: str
    inputs: tuple[tuple[str, str, str], ...]
    data: tuple[dict, ...]
    view: str = "slices"
    seed: str | None = None
    band: tuple[int, ...] = (8,)
    labels: tuple[str, ...] = ()
    place: tuple[str, str, str] | None = None
    command: str = ""
    node: str = ""
    extra_facts: tuple[tuple[str, str], ...] = field(default=())


def resolve(arg: str, out: Path | None = None) -> str:
    """An argument with its placeholders resolved (``{out:...}`` into ``out``)."""

    def sub(m: re.Match[str]) -> str:
        key = m.group(1)
        if key.startswith("in:"):
            import larmorx_testdata as td

            return str(td.get(key[3:]))
        if key.startswith("out:"):
            if out is None:
                raise ValueError("an output placeholder without an output directory")
            return str(out / key[4:])
        raise KeyError(f"unknown placeholder {{{key}}}")

    return re.sub(r"\{([^{}]+)\}", sub, arg)


def runs(spec: Spec) -> list[tuple[str, str, str, str, dict, bool]]:
    """``(label, role, panel label, sub, env, is_ants)`` of every run of an example."""
    out = [
        ("ants-1", "original", "ANTs 2.6.5, 1 thread", "as built (GCC 14.3), glibc's maths",
         {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}, True),
        ("ants-1-cr", "reference", "ANTs 2.6.5, correctly rounded maths",
         "1 thread, CORE-MATH preloaded", {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}, True),
    ]  # fmt: skip
    for t in spec.band:
        out.append(
            (f"ants-{t}", "original", f"ANTs 2.6.5, {t} threads", "as built",
             {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(t)}, True)
        )  # fmt: skip
    out += [
        ("larmorx", "larmorx", "larmorx (port)", f"reproduce mode, {LX_THREADS} threads",
         {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(LX_THREADS),
          "LARMORX_UPSTREAM_DEFECTS": "reproduce"}, False),
        ("larmorx-1", "larmorx", "larmorx (port)", "reproduce mode, 1 thread",
         {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "LARMORX_UPSTREAM_DEFECTS": "reproduce"},
         False),
        ("larmorx-fix", "larmorx", "larmorx (port)", f"default mode, {LX_THREADS} threads",
         {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(LX_THREADS),
          "LARMORX_UPSTREAM_DEFECTS": "fix"}, False),
    ]  # fmt: skip
    if spec.seed is not None:
        for r in out:
            r[4]["ANTS_RANDOM_SEED"] = spec.seed
    return out


def _compute(spec: Spec):
    def compute(e: fcache.Entry) -> None:
        from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

        b, lib = ants_bin(), crmath_preload()
        if b is None or lib is None:
            raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload")
        for label, _role, _l, _s, env, is_ants in runs(spec):
            out = e.dir / label
            out.mkdir(exist_ok=True)
            args = [resolve(a, out) for a in spec.args]
            env = dict(env)
            if label.endswith("-cr"):
                env["LD_PRELOAD"] = str(lib)
            argv = (
                [str(b / spec.program), *args]
                if is_ants
                else fcache.larmorx_cli("ants", spec.program, *args)
            )
            e.run(label, argv, env=env, unset=common.CLEAN_ENV)
            if spec.place is not None:
                moving, fixed, name = spec.place
                e.run(
                    f"{label}-placed",
                    [str(b / "antsApplyTransforms"), "-d", "3", "-i", resolve(moving),
                     "-r", resolve(fixed), "-t", str(out / name), "-n", "Linear",
                     "-o", str(out / "placed.nii.gz")],
                    env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"},
                    unset=common.CLEAN_ENV,
                )  # fmt: skip
        for i, (_pid, path, label) in enumerate(spec.inputs):
            e.input(f"input {i + 1} ({label})", Path(resolve(path)))
        e.finish(
            versions={"ants": ants_version()},
            threads={"ants": [1, *spec.band], "ants_crmath": 1, "larmorx": [LX_THREADS, 1]},
            settings=f"a data-panel case of {spec.program}: ANTs 2.6.5 at 1 thread (as built and "
            f"with CORE-MATH) and at {', '.join(map(str, spec.band)) or 'no other'} threads; "
            f"larmorx in reproduce mode at {LX_THREADS} and 1 threads, in its default mode at "
            f"{LX_THREADS}"
            + (
                "; each transform placed by ANTs' antsApplyTransforms (linear)"
                if spec.place
                else ""
            ),
        )

    return compute


def entry(ctx: core.Context, spec: Spec) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), spec.tool, spec.case, f"{spec.title} (data panel)", _compute(spec)
    )


def _file(e: fcache.Entry, label: str, name: str) -> Path:
    return e.dir / label / name


DIFFS = [
    ("lx-ants", "larmorx", "ants-1", "main", "larmorx − ANTs 2.6.5, 1 thread, as built",
     common.ROLE_LX),
    ("lx-cr", "larmorx", "ants-1-cr", "main", "larmorx − ANTs 2.6.5, correctly rounded maths",
     common.ROLE_LX),
    ("lx1-lx", "larmorx-1", "larmorx", "check",
     f"larmorx, 1 thread − {LX_THREADS} threads (reproduce)", common.ROLE_SELF),
    ("fix-lx", "larmorx-fix", "larmorx", "check", "larmorx, default − reproduce mode",
     common.ROLE_SELF),
]  # fmt: skip


def verdicts(diffs: list[dict], defects_named: list[str]) -> None:
    """Each comparison's verdict from its metrics (``docs/figures.md``, the verdicts):
    bit-identical; identical with correctly rounded maths only where larmorx − ANTs with
    CORE-MATH is bit-identical; a D13 correction where larmorx's default differs from its
    reproduce mode and names a defect; ANTs against itself is the yardstick (no verdict)."""
    same = common.identical(diffs)
    per_out = {x["id"]: {o: m["identical"] for o, m in x["metrics"].items()} for x in diffs}

    def differing(i: str) -> str:
        return ", ".join(o for o, s in per_out[i].items() if not s)

    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = (
                "The same bytes: ANTs' thread count changes nothing in this call."
                if same[i]
                else f"ANTs' own result moves with its thread count ({differing(i)}): the "
                "yardstick for any difference above."
            )
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "lx-ants": "larmorx gives the unmodified ANTs program's bytes.",
                "lx-cr": "larmorx gives ANTs' bytes with correctly rounded maths.",
                "lx1-lx": "larmorx's result does not depend on its thread count.",
                "fix-lx": "No correction of the defect register changes this call.",
            }.get(i, "The same bytes.")
        elif i == "lx-ants" and same.get("lx-cr"):
            x["verdict"] = "identical-cr"
            x["reason"] = (
                f"Only where glibc's maths is not correctly rounded ({differing(i)}): with "
                "CORE-MATH preloaded, ANTs gives larmorx's bytes."
            )
        elif i == "fix-lx" and defects_named:
            x["verdict"] = "d13"
            x["reason"] = (
                f"larmorx's default corrects {', '.join(defects_named)} "
                "([register](repo:docs/findings/upstream-defects.md)); reproduce mode gives "
                f"ANTs' result. Changed: {differing(i)}."
            )
        else:
            x["verdict"] = "open"
            x["reason"] = f"Not accounted for by the other runs of this example ({differing(i)})."


def _named(e: fcache.Entry) -> list[str]:
    """The defects larmorx's default named in this entry's run (its standard error)."""
    log = e.dir / "larmorx-fix.log"
    text = log.read_text(encoding="utf-8") if log.is_file() else ""
    return sorted(set(re.findall(r"\((ANTS-\d+|ITK-\d+)\)", text)))


def _window_of(ex, pid: str) -> list[float]:
    from larmorx_validation.evidence import example as exm

    v = ex.values(ex.key(pid, "all"))
    m = ex.m if v.ndim == 3 else np.broadcast_to(ex.m[..., None], v.shape)
    w = exm.windows(v[m])[0]
    return [w["lo"], w["hi"]]


def example(ctx: core.Context, spec: Spec, e: fcache.Entry | None = None) -> dict:
    """The worked example of ``spec`` (its cache entry computed if missing)."""
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = e or entry(ctx, spec)
    info = e.info()
    grid_path = resolve(spec.grid)
    img = nib.load(grid_path)
    rs = runs(spec)
    names = {oid: name for oid, name, _l, _u in spec.outputs}
    if spec.place is not None:
        names = {oid: "placed.nii.gz" for oid in names}
    arrays: dict[str, dict[str, np.ndarray]] = {}
    scales: dict[str, tuple] = {}
    for label, *_rest in rs:
        arrays[label] = {}
        for oid, name in names.items():
            a, scale, inter = common.stored(_file(e, label, name))
            arrays[label][oid] = a
            scales[label] = (scale, inter)
    ins = {}
    for pid, path, _label in spec.inputs:
        ins[pid] = common.stored(resolve(path))
    show = common.unmasked(
        *(a for a, _s, _i in ins.values()), *(a for d in arrays.values() for a in d.values())
    )
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show, view=spec.view)
    for pid, path, label in spec.inputs:
        a, scale, inter = ins[pid]
        ex.add(exm.Panel(pid, "input", label, Path(resolve(path)).name, {"all": a},
                         scale=scale, inter=inter))  # fmt: skip
    first = spec.inputs[0][0]
    overlay = None
    if spec.labels:
        overlay = {o: {"on": first, "window": _window_of(ex, first)} for o in spec.labels}
    shown = ["ants-1", "ants-1-cr", "larmorx", "larmorx-fix", *(f"ants-{t}" for t in spec.band)]
    by_label = {r[0]: r for r in rs}
    for label in shown:
        _l, role, text, sub, _env, _a = by_label[label]
        scale, inter = scales[label]
        ex.add(exm.Panel(label, role, text, sub, arrays[label], scale=scale, inter=inter,
                         overlay=overlay))  # fmt: skip
    outputs = [{"id": oid, "label": lab, "unit": unit} for oid, _n, lab, unit in spec.outputs]
    dd = common.d
    diffs = [dd(*x) for x in DIFFS]
    for t in spec.band:
        diffs.append(dd(f"ants{t}-ants1", f"ants-{t}", "ants-1", "variability",
                        f"ANTs 2.6.5, {t} threads − 1 thread", common.ROLE_ORIG))  # fmt: skip
    common.compare_all(ex, diffs, outputs, full={"larmorx-1": arrays["larmorx-1"]})
    verdicts(diffs, _named(e))
    out0 = outputs[0]["id"]
    if spec.view == "slices":
        at = common.slice_voxel(ex, "larmorx", "ants-1", out0)
    else:
        at, _k = common.largest_in_slice(ex, "larmorx", "ants-1", out0)
    default = {
        "output": out0,
        "plane": "axial",
        "voxel": at,
        "panels": [first, "ants-1", "larmorx", "larmorx-fix"],
        "diffs": ["lx-ants", "lx-cr", "fix-lx"],
        "compare": {"a": "ants-1", "b": "larmorx"},
    }
    differs = not all(m["identical"] for m in diffs[0]["metrics"].values())
    res = common.finish(
        ctx, ex, diffs, copy.deepcopy(outputs), default=default, window_panel="larmorx",
        caption="the crosshair " + ("where larmorx differs most from ANTs as built"
                                    if differs else "at the centre of the field of view")
        + ("; the three slices through it are stored at full resolution."
           if spec.view == "slices" else "."),
    )  # fmt: skip
    vers = info.get("versions", {})
    data = "; ".join(f"{x['dataset']}, {x['subject']} ({x['licence']})" for x in spec.data)
    facts = [
        {"label": "Data", "text": data + "; the whole field of view shown, only margins where "
         "every array is 0 left out"},
        {"label": "Command", "text": f"`{spec.command}`"},
        {"label": "Programs", "text": f"{vers.get('ants', '?')}; larmorx at commit "
         f"`{info['larmorx_commit'][:12]}`"},
        {"label": "Grid", "text": f"{' × '.join(map(str, img.shape))} voxels of "
         f"{' × '.join(f'{z:.3g}' for z in img.header.get_zooms()[:3])} mm; shown "
         f"{' × '.join(map(str, grid.shape))}"
         + (", three full-resolution slices" if spec.view == "slices" else ", every slice")},
        *({"label": k, "text": v} for k, v in spec.extra_facts),
    ]  # fmt: skip
    if spec.node:
        facts.insert(1, {"label": "Node", "text": spec.node})
    n_vox = int(np.prod(arrays["ants-1"][out0].shape))
    return {
        "id": spec.id,
        "title": spec.title,
        "summary": spec.summary,
        "facts": facts,
        **res,
        "chain": None,
        "metrics_note": f"Δ is A − B over all {n_vox:,} values of each output"
        + (" (the moving image placed on the fixed grid through each run's transform)"
           if spec.place else "") + ".",
        "provenance": common.provenance(info, f"{spec.tool}/{e.case}", list(spec.data)),
    }  # fmt: skip


def transform_table(ctx: core.Context, spec: Spec, tid: str, title: str) -> dict:
    """The transform of every run of a transform program's example, parameter by parameter."""
    e = entry(ctx, spec)
    name = spec.place[2] if spec.place else spec.outputs[0][1]
    rows, n = [], 0
    for label, _role, text, sub, _env, _a in runs(spec):
        params, fixed, cls = common.itk_linear(_file(e, label, name))
        n = max(n, len(params))
        rows.append(
            {
                "head": f"{text} ({sub})",
                "cells": [
                    common.cell(cls),
                    *(common.cell(float(v), "f64") for v in params),
                    common.cell(" ".join(f"{float(v):.9g}" for v in fixed)),
                ],
            }
        )
    columns = [
        "Run",
        "Class",
        *({"label": f"p{i}", "num": True} for i in range(n)),
        "Centre (fixed parameters)",
    ]
    return common.table(
        tid,
        title,
        columns,
        rows,
        note="Every parameter as stored (the shortest float64 that reads back exactly).",
    )


__all__ = ["DIFFS", "LX_THREADS", "Spec", "entry", "example", "runs", "transform_table"]
