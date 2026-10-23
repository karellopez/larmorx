# SPDX-License-Identifier: Apache-2.0
"""Worked examples of the corrections decided on 2026-10-23 (rule A: ANTS-17, ANTS-40,
ANTS-44): one real input on which ANTs' defect changes its result, run by the ANTs 2.6.5
oracle at one thread (as built, and with CORE-MATH's correctly rounded maths preloaded) and
by larmorx in its default mode (the correction) and its ``reproduce`` mode (ANTs' result), and,
where the pipeline ran the call, fMRIPrep's own output.

Each example shows the correction as a deliberate correction: larmorx's default against its
reproduce mode (the d13 verdict, decided from the metrics), its reproduce mode against ANTs
(bit for bit, or bit for bit with correctly rounded maths), ANTs' own variability beside them.
A tool's module describes its example with a :class:`Spec` and calls :func:`example`.
"""

from __future__ import annotations

import copy
import shlex
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache

#: larmorx's threads in these runs (its results do not depend on them).
LX_THREADS = 6


@dataclass(frozen=True)
class Spec:
    """One correction example.

    ``args`` are the ANTs program's arguments with ``{in:<key>}`` (a file of ``inputs``) and
    ``{out:<id>}`` (the output ``id``) placeholders. ``inputs`` maps a key to (label, path);
    ``panels`` names the inputs shown (they must be on the outputs' grid). ``show`` is the
    display mask on that grid (the brain, dilated); ``fmriprep`` the pipeline's own outputs
    (output id -> path), shown and compared when given."""

    tool: str
    case: str
    entry: str
    title: str
    summary: str
    program: str
    args: tuple[str, ...]
    inputs: dict[str, tuple[str, Path]]
    panels: tuple[tuple[str, str, str], ...]  # (input key, label, sub)
    outputs: tuple[dict, ...]
    data: tuple[dict, ...]
    show: Callable[[], np.ndarray]
    reason: str
    effect: str | Callable[[fcache.Entry], str] = ""
    node: str = ""
    fmriprep: dict[str, Path] = field(default_factory=dict)
    settings: str = ""


def _argv(spec: Spec, out: Callable[[str], str]) -> list[str]:
    def sub(a: str) -> str:
        for k, (_label, p) in spec.inputs.items():
            a = a.replace("{in:" + k + "}", str(p))
        for o in spec.outputs:
            a = a.replace("{out:" + o["id"] + "}", out(o["id"]))
        return a

    return [sub(a) for a in spec.args]


def entry(ctx: core.Context, spec: Spec) -> fcache.Entry:
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    def compute(e: fcache.Entry) -> None:
        b, lib = ants_bin(), crmath_preload()
        if b is None or lib is None:
            raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload")
        for _k, (label, p) in spec.inputs.items():
            e.input(label, p)
        for o, p in spec.fmriprep.items():
            e.input(f"fMRIPrep's {o}", p)

        def outs(run: str) -> Callable[[str], str]:
            return lambda oid: str(e.dir / f"{run}-{oid}.nii.gz")

        for run, env in (
            ("ants-1", {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}),
            ("ants-1-cr", {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "LD_PRELOAD": str(lib)}),
        ):
            e.run(run, [str(b / spec.program), *_argv(spec, outs(run))], env=env,
                  unset=common.CLEAN_ENV)  # fmt: skip
        for run, mode in (("larmorx", "fix"), ("larmorx-reproduce", "reproduce")):
            e.run(
                run,
                fcache.larmorx_cli("ants", spec.program, *_argv(spec, outs(run))),
                env={
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(LX_THREADS),
                    "LARMORX_UPSTREAM_DEFECTS": mode,
                },
                unset=common.CLEAN_ENV,
            )
        e.finish(
            versions={"ants": ants_version()},
            command=shlex.join([spec.program, *_argv(spec, lambda o: f"{o}.nii.gz")]),
            threads={"ants": 1, "larmorx": LX_THREADS},
            settings=spec.settings,
        )

    return fcache.entry(ctx.figures_context(), spec.tool, spec.case, spec.title, compute)


RUNS = (
    ("larmorx", "larmorx", "larmorx (port)", "default mode: {entry} corrected"),
    ("larmorx-reproduce", "larmorx", "larmorx (port)", "reproduce mode: ANTs' behaviour"),
    ("ants-1-cr", "reference", "ANTs 2.6.5, 1 thread, correctly rounded maths",
     "the same binary, CORE-MATH preloaded"),
    ("ants-1", "original", "ANTs 2.6.5, 1 thread, as built",
     "the repository's oracle build (GCC 14.3), glibc's maths"),
)  # fmt: skip


def example(ctx: core.Context, spec: Spec, reg: dict) -> dict:
    """The worked example of ``spec``'s correction (its cache entry computed if missing)."""
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = entry(ctx, spec)
    info = e.info()
    first = spec.outputs[0]["id"]
    ref_img = nib.load(str(e.dir / f"ants-1-{first}.nii.gz"))
    show = spec.show()
    grid = core.Grid.around(ref_img, show)
    ex = exm.Example(ctx, grid, show)
    for key, label, sub in spec.panels:
        ex.add(exm.Panel(key, "input", label, sub, {"all": common.load(str(spec.inputs[key][1]))}))
    for pid, role, label, sub in RUNS:
        arrays = {
            o["id"]: common.load(str(e.dir / f"{pid}-{o['id']}.nii.gz")) for o in spec.outputs
        }
        ex.add(exm.Panel(pid, role, label, sub.format(entry=spec.entry), arrays))
    if spec.fmriprep:
        ex.add(
            exm.Panel(
                "fmriprep",
                "fmriprep",
                "fMRIPrep 25.2.6, as it ran",
                "ANTs 2.6.2 in its image",
                {o: common.load(str(p)) for o, p in spec.fmriprep.items()},
            )
        )
    d = common.d
    diffs = [
        d("fix-repro", "larmorx", "larmorx-reproduce", "main",
          f"larmorx default − reproduce mode ({spec.entry})", "larmorx's correction"),
        d("fix-ants", "larmorx", "ants-1", "main", "larmorx default − ANTs 2.6.5, as built",
          common.ROLE_LX),
        d("repro-cr", "larmorx-reproduce", "ants-1-cr", "main",
          "larmorx reproduce − ANTs 2.6.5, correctly rounded maths", common.ROLE_LX),
        d("repro-ants", "larmorx-reproduce", "ants-1", "main",
          "larmorx reproduce − ANTs 2.6.5, as built", common.ROLE_LX),
        d("cr-ants", "ants-1-cr", "ants-1", "variability",
          "ANTs 2.6.5: correctly rounded maths − glibc's", common.ROLE_ORIG),
    ]  # fmt: skip
    if spec.fmriprep:
        diffs.append(
            d("fmriprep-ants", "fmriprep", "ants-1", "variability",
              "fMRIPrep's own output − ANTs 2.6.5, 1 thread", common.ROLE_ORIG)
        )  # fmt: skip
    outputs = copy.deepcopy(list(spec.outputs))
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    if same["fix-repro"]:
        raise AssertionError(f"{spec.tool}/{spec.case}: the correction changes nothing here")
    link = {"label": spec.entry, "href": reg[spec.entry]["link"]} if spec.entry in reg else None
    for x in diffs:
        i = x["id"]
        x["links"] = [link] if link and i in ("fix-repro", "fix-ants") else []
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = (
                "The same bytes."
                if same[i]
                else {
                    "cr-ants": "Where glibc's exp, log, sin or cos is not correctly rounded for a "
                    "value the run meets: ANTs' own result moves with the maths library.",
                    "fmriprep-ants": "fMRIPrep's run used its own ANTs build and thread count: "
                    "ANTs' own variability.",
                }[i]
            )
        elif i in ("fix-repro", "fix-ants"):
            x["verdict"] = "d13"
            x["reason"] = spec.reason
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = "Reproduce mode gives ANTs' result, the defect included."
        elif i == "repro-ants" and same["repro-cr"]:
            x["verdict"] = "identical-cr"
            x["reason"] = (
                "Only where glibc's maths is not correctly rounded: with CORE-MATH preloaded, "
                "ANTs gives larmorx's reproduce-mode output bit for bit (the row above)."
            )
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
    at, _k = common.largest_in_slice(ex, "larmorx", "larmorx-reproduce", first)
    inputs = [k for k, _l, _s in spec.panels]
    default = {
        "output": first,
        "plane": "axial",
        "voxel": at,
        "panels": [*inputs[:1], "ants-1", "larmorx", "larmorx-reproduce"],
        "diffs": ["fix-repro", "repro-cr"],
        "compare": {"a": "ants-1", "b": "larmorx"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="ants-1",
        caption="the crosshair where larmorx's correction changes the output most in it.",
    )  # fmt: skip
    fx = next(x for x in diffs if x["id"] == "fix-repro")
    effect = "; ".join(
        f"{o['label']}: {m['differ']:,} of {m['voxels']:,} values change, by at most "
        f"{m['max_abs']:.3g}"
        for o in outputs
        if (m := fx["metrics"].get(o["id"]))
    )
    title = reg.get(spec.entry, {}).get("title", "")
    lead = spec.effect(e) if callable(spec.effect) else spec.effect
    facts = [
        {"label": "Data", "text": "; ".join(
            f"{x['dataset']}" + (f", {x['subject']}" if x.get("subject") else "")
            + f" ({x['licence']})" for x in spec.data)},
        *([{"label": "Node", "text": spec.node}] if spec.node else []),
        {"label": "Command", "text": "`" + info.get("command", "") + "`"},
        {
            "label": "Defect",
            "text": (f"[{spec.entry}](repo:{link['href']}): {title}" if link else spec.entry)
            + "; corrected by default (decided 2026-10-23), `reproduce` gives ANTs' result",
        },
        {"label": "Effect", "text": f"{lead}; {effect}" if lead else effect},
        {"label": "Programs", "text": f"{info.get('versions', {}).get('ants', '?')}, one thread; "
         f"larmorx at commit `{info['larmorx_commit'][:12]}`, {LX_THREADS} threads"},
        {"label": "Grid", "text": common.describe_grid(ref_img, grid.shape)},
    ]  # fmt: skip
    return {
        "id": spec.entry.lower(),
        "title": spec.title,
        "summary": spec.summary,
        "facts": facts,
        **out,
        "chain": None,
        "metrics_note": (
            f"Δ is A − B, over all {int(np.prod(ref_img.shape)):,} voxels of each output file; "
            "the rows *the original against itself* are ANTs' own variability on this input."
        ),
        "provenance": common.provenance(info, f"{spec.tool}/{spec.case}", spec.data),
    }


__all__ = ["LX_THREADS", "Spec", "entry", "example"]
