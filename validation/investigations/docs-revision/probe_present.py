# SPDX-License-Identifier: Apache-2.0
"""The presenter's probe and regression gate, for the rollout's evidence pages (the reviewer's
round 9, finding 2, and round 10's chunk X).

**The probe** dry-runs :func:`larmorx_validation.evidence.present.example` over every worked
example of every evidence page of a site checkout that is not yet presented, each page described
as ``validation/programs.toml`` describes it (its software and the original's version from its
group, its program or programs, its ports from their records), and lists every shape the
presenter refuses: every panel, comparison and run of an example, not only the first. A page
about to be rewritten must show 0 refused.

**The regression gate** (``--ref <commit>``) runs each presented page's own module's ``present``
on that page's JSON as the site published it before the revision (``git show <before>:...``, the
site's main at b768293 by default) with this checkout's code and with the code at ``<commit>``
(a temporary git worktree), and lists every difference: a change to the presenter must change
nothing on the pages already presented, or only what ``--intended`` lists (one JSON pointer
prefix per line, ``<page> <pointer>``).

    PYTHONPATH=python:validation/src python -P probe_present.py <site> [--page <name>]...
        [--ref <commit> [--before b768293] [--intended <file>]]
"""

from __future__ import annotations

import argparse
import copy
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
#: Not evidence pages of programs.
OTHER = {"summary", "difftest", "panel", "sizes"}
#: Groups whose ``original`` names no version (programs.toml): the software and version the
#: pages compare with, from their records.
GROUP_ORIGINAL = {"fmriprep": ("fMRIPrep", "25.2.6"), "larmorx": ("nibabel", "5.4.2")}


def pages(repo: Path) -> dict[str, dict]:
    """Every evidence page (its JSON's name) as programs.toml describes it."""
    d = tomllib.loads((repo / "validation/programs.toml").read_text(encoding="utf-8"))
    groups = {g["id"]: g for g in d["group"]}
    out: dict[str, dict] = {}
    for p in d["program"]:
        addr = p.get("evidence")
        if not addr:
            continue
        name = addr.strip("/").split("/")[-1]
        g = groups[p["group"]]
        if p["group"] in GROUP_ORIGINAL:
            sw, ver = GROUP_ORIGINAL[p["group"]]
        else:
            m = re.match(r"^(\w+) ([0-9][\w.]*)", g["original"])
            if not m:
                raise SystemExit(f"{p['group']}: no version in {g['original']!r}")
            sw, ver = m.group(1), m.group(2)
        e = out.setdefault(name, {"software": sw, "version": ver, "programs": [], "ports": []})
        e["programs"].append(p["name"])
        for port in p.get("records", {}):
            if port not in e["ports"]:
                e["ports"].append(port)
    return out


def page_of(present, name: str, e: dict):
    ports = [p for p in e["ports"] if p in ("replica", "clean-room")] or ["replica"]
    progs = e["programs"]
    return present.Page(
        software=e["software"],
        program=progs[0] if len(progs) == 1 else "",
        version=e["version"],
        port=ports[0],
        programs=tuple(progs) if len(progs) > 1 else (),
    )


def probe(site: Path, only: list[str]) -> int:
    from larmorx_validation.evidence import present

    described = pages(REPO)
    refused = 0
    for f in sorted((site / "static/evidence").glob("*.json")):
        name = f.stem
        if name in OTHER or (only and name not in only):
            continue
        doc = json.loads(f.read_text(encoding="utf-8"))
        if doc.get("presentation") == "writing-guide":
            print(f"{name:24s} presented (its module's present; see --ref)")
            continue
        if name not in described:
            print(f"{name:24s} not in programs.toml")
            refused += 1
            continue
        page = page_of(present, name, described[name])
        ok, msgs = 0, []
        for ex in doc.get("examples", []):
            errs: list[str] = []
            try:
                present.example(page, copy.deepcopy(ex), shows="x", verdicts=[], errors=errs)
            except Exception as exc:  # every refusal is listed
                errs.append(f"{type(exc).__name__}: {exc}")
            if errs:
                msgs += [f"{ex['id']}: {m}" for m in dict.fromkeys(errs)]
            else:
                ok += 1
        n = len(doc.get("examples", [])) - ok
        refused += n
        print(f"{name:24s} examples {len(doc.get('examples', []))}: {ok} pass, {n} refused")
        for m in msgs:
            print(f"    {m[:200]}")
    print(f"{refused} examples refused")
    return refused


def _dry(code: Path, module: str, src: Path, dst: Path) -> None:
    env = dict(os.environ, PYTHONPATH=f"{code}/python:{code}/validation/src")
    script = REPO / "validation/investigations/docs-revision/present_dry_run.py"
    subprocess.run(
        [sys.executable, "-P", str(script), module, str(src), str(dst)],
        env=env,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )


def _diff(a, b, p: str, out: list[str]) -> None:
    if isinstance(a, dict) and isinstance(b, dict):
        for k in dict.fromkeys([*a, *b]):
            _diff(a.get(k), b.get(k), f"{p}/{k}", out)
    elif isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        for i, (x, y) in enumerate(zip(a, b, strict=True)):
            _diff(x, y, f"{p}/{i}", out)
    elif a != b:
        out.append(p)


def regress(site: Path, ref: str, before: str, intended: Path | None, only: list[str]) -> int:
    from larmorx_validation import evidence

    allowed = defaultdict(list)
    if intended:
        for line in intended.read_text(encoding="utf-8").splitlines():
            if line.strip() and not line.startswith("#"):
                pg, ptr = line.split(None, 1)
                allowed[pg].append(ptr.strip())
    total = 0
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        old = tmp / "ref"
        subprocess.run(
            ["git", "-C", str(REPO), "worktree", "add", "--detach", str(old), ref],
            check=True,
            capture_output=True,
        )
        # the compiled module is not in git: the reference worktree uses this checkout's
        so = REPO / "python/larmorx/_core.abi3.so"
        if so.exists():
            (old / "python/larmorx/_core.abi3.so").write_bytes(so.read_bytes())
        try:
            for f in sorted((site / "static/evidence").glob("*.json")):
                name = f.stem
                if name in OTHER or (only and name not in only):
                    continue
                if json.loads(f.read_text(encoding="utf-8")).get("presentation") != "writing-guide":
                    continue
                src = tmp / f"{name}.before.json"
                src.write_bytes(
                    subprocess.run(
                        ["git", "-C", str(site), "show", f"{before}:static/evidence/{name}.json"],
                        check=True,
                        capture_output=True,
                    ).stdout
                )
                module = evidence.TOOLS[name]
                try:
                    _dry(old, module, src, tmp / f"{name}.ref.json")
                except subprocess.CalledProcessError:  # presented after <ref>: no baseline
                    print(f"{name:24s} presented after {ref}: nothing to compare")
                    continue
                try:
                    _dry(REPO, module, src, tmp / f"{name}.now.json")
                except subprocess.CalledProcessError as exc:  # its present refuses: listed
                    total += 1
                    print(f"{name:24s} its present fails: {exc.stderr.strip().splitlines()[-1]}")
                    continue
                a = json.loads((tmp / f"{name}.ref.json").read_text(encoding="utf-8"))
                b = json.loads((tmp / f"{name}.now.json").read_text(encoding="utf-8"))
                diffs: list[str] = []
                _diff(a, b, "", diffs)
                bad = [d for d in diffs if not any(d.startswith(x) for x in allowed[name])]
                total += len(bad)
                print(f"{name:24s} {len(diffs)} differences, {len(bad)} not intended")
                for d in bad[:20]:
                    print(f"    {d}")
        finally:
            subprocess.run(
                ["git", "-C", str(REPO), "worktree", "remove", "--force", str(old)],
                check=False,
                capture_output=True,
            )
    print(f"{total} differences not intended")
    return total


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("site", type=Path)
    ap.add_argument("--page", action="append", default=[])
    ap.add_argument("--ref")
    ap.add_argument("--before", default="b768293")
    ap.add_argument("--intended", type=Path)
    a = ap.parse_args()
    if a.ref:
        sys.exit(1 if regress(a.site, a.ref, a.before, a.intended, a.page) else 0)
    sys.exit(1 if probe(a.site, a.page) else 0)


if __name__ == "__main__":
    main()
