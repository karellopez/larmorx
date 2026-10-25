# SPDX-License-Identifier: Apache-2.0
"""Permanent cases: a difference found by random testing, shrunk, kept as a recipe
(``validation/difftest/cases/<tool>/<id>.json``) that the tool's parity suite runs as a regular
case from then on (``difftest/<id>``, in the tiers its file names).

A file holds the shrunk recipe, where it was found (date, seed, case number, the checks that
failed), what was done (``status``: ``fixed`` in larmorx, ``register`` for a defect of the
original with its ``defect`` ID and rule, ``open`` for a clean-room original's difference
reported for its agent), the case's expectation (``expect``: ``pass``, or an expected
divergence or error with its ``reason``) and its tier. The inputs are rebuilt from the recipe
(:mod:`~larmorx_validation.difftest.inputs`), the original's runs replay from the oracle cache.
"""

from __future__ import annotations

import json
from pathlib import Path

from larmorx_validation.difftest import inputs, tools
from larmorx_validation.parity.harness import Case

TIERS = ("smoke", "standard", "full")


def files(tool: str) -> list[Path]:
    d = inputs.CASES / tool
    return sorted(d.glob("*.json")) if d.is_dir() else []


def load(tool: str) -> list[dict]:
    return [json.loads(p.read_text(encoding="utf-8")) for p in files(tool)]


def _kw(tool_name: str, p: dict) -> dict:
    """The payload's own fields for the case's tier and expectation."""
    t = tools.get(tool_name)
    fields = getattr(t, "payload_fields", ("tier", "expect", "reason"))
    kw = {}
    if "tier" in fields:
        kw["tier"] = p.get("tier", "smoke")
    expect = p.get("expect", "pass")
    if "expect" in fields and expect != "pass":
        kw["expect"] = expect
    if "expect_error" in fields and expect == "error":
        kw["expect_error"] = True
    if "reason" in fields and p.get("reason"):
        kw["reason"] = p["reason"]
    return kw


def cases(tool_name: str, tier: str = "full", implementation: str | None = None) -> list[Case]:
    """The permanent cases of ``tool_name`` in ``tier`` (and the tiers below it), for
    ``implementation`` (``None``: the tool's first)."""
    t = tools.get(tool_name)
    impl = implementation or t.implementations[0]
    rank = TIERS.index(tier)
    out = []
    for p in load(tool_name):
        if TIERS.index(p.get("tier", "smoke")) > rank:
            continue
        if impl not in p.get("implementations", t.implementations):
            continue
        c = t.case(
            p["recipe"],
            impl,
            case_prefix=f"case/{tool_name}/{p['id']}",
            cid=f"difftest/{p['id']}",
            description=p["description"],
            **_kw(tool_name, p),
        )
        out.append(Case(c.id, "difftest", p["description"], c.payload))
    return out


def payloads(tool_name: str, tier: str = "full", implementation: str | None = None) -> list:
    """The suites' own case objects (``AntsCase``, ``RegCase``, ...) of the permanent cases."""
    return [c.payload for c in cases(tool_name, tier, implementation)]


#: parity suite -> (difftest tool, implementation): where each tool's permanent cases run.
SUITES: dict[str, tuple[str, str]] = {
    **{
        name: (name, "port")
        for name in (
            "ants-image-math",
            "ants-threshold-image",
            "ants-multiply-images",
            "ants-smooth-image",
            "ants-resample-image",
            "ants-resample-image-by-spacing",
            "ants-n4",
            "ants-denoise",
            "ants-registration",
            "ants-ai",
            "ants-affine-initializer",
            "ants-atropos",
            "ants-apply-transforms",
            "nifti-io",
            "resample-series",
        )
    },
    "itk-hdf5": ("itk-transforms", "port"),
    "afni-tshift": ("afni-tshift", "original"),
    "afni-tshift-replica": ("afni-tshift", "replica"),
    "afni-automask": ("afni-automask", "original"),
    "afni-automask-replica": ("afni-automask", "replica"),
    "afni-unifize": ("afni-unifize", "original"),
    "afni-unifize-replica": ("afni-unifize", "replica"),
    "mri-hmc": ("mri-hmc", "original"),
    "fsl-mcflirt-replica": ("fsl-mcflirt", "replica"),
}


def attach(suite) -> None:
    """Adds the permanent cases of ``suite``'s tool to its cases (every tier: each case's own
    tier filters it), and runs those whose payload is a difftest one (a recorded run made live:
    head motion) through the tool's own runner. Nothing changes for a suite without any."""
    if suite.name not in SUITES:
        return
    tool_name, impl = SUITES[suite.name]
    if not files(tool_name):
        return
    base_cases, base_run = suite.cases, suite.run_case

    def with_permanent(tier: str):
        return [*base_cases(tier), *cases(tool_name, tier, impl)]

    def run_case(case, checks):
        payload = case.payload
        if isinstance(payload, dict) and "recipe" in payload:
            return tools.get(tool_name).run(impl)(case, checks)
        return base_run(case, checks)

    suite.cases = with_permanent
    suite.run_case = run_case


def promote(finding: Path, case_id: str, *, tier: str = "smoke", expect: str = "pass", reason: str = "", status: str = "fixed", defect: str | None = None, rule: str | None = None, description: str = "", out_root: Path | None = None) -> Path:
    """Writes the permanent case of a finding (its shrunk recipe)."""
    f = json.loads(Path(finding).read_text(encoding="utf-8"))
    root = out_root or inputs.CASES
    d = root / f["tool"]
    d.mkdir(parents=True, exist_ok=True)
    recipe = f["shrunk"]
    t = tools.get(f["tool"])
    doc = {
        "id": case_id,
        "tool": f["tool"],
        "suite": t.suite,
        "implementations": [f["implementation"]],
        "description": description or f"random testing: {', '.join(f['signature'])}",
        "found": {
            "seed": f["seed"],
            "n": f["n"],
            "signature": f["signature"],
            "detail": (f.get("shrunk_detail") or f.get("detail", ""))[:800],
            "shrink_moves": f.get("shrink_moves", []),
        },
        "status": status,
        "defect": defect,
        "rule": rule,
        "tier": tier,
        "expect": expect,
        "reason": reason,
        "recipe": recipe,
    }
    path = d / f"{case_id}.json"
    path.write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    return path


__all__ = ["SUITES", "attach", "cases", "files", "load", "payloads", "promote"]
