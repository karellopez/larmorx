# SPDX-License-Identifier: Apache-2.0
"""The field maps' part B's contract audit (relay 22): the ``_core`` stubs, the API pages' rows,
the PROVENANCE lines.

1. Every function ``crates/larmorx-py/src`` registers (``wrap_pyfunction!``) has a stub in
   ``python/larmorx/_core.pyi`` and every stub a function; where the Rust function declares a
   ``#[pyo3(signature = (...))]``, the stub's parameter names, order and defaults are the same.
2. Every public function of ``lx.sdcflows``, ``lx.niworkflows``, ``lx.ndimage``, ``lx.skimage``,
   ``lx.bids`` and larmorprepx's field-map modules is named on an API page (``docs/api/``), and
   each of its parameters appears in a section (between headings) that names it.
3. Every source file the part added or changed (from the merge base with ``main``) under
   ``crates/`` and ``python/larmorx/`` is named in a crate's ``PROVENANCE.md`` (by its path, its
   module path or its stem), except larmorx's own glue (``__init__.py``, the bindings, tests,
   the defects table, the stubs).

Run from the repository with the package importable::

    PYTHONPATH=python python -P validation/investigations/sdcflows-fieldmap-b/relay22/audit_contract.py

Prints each gap and a summary line per check; exit 0 when nothing is missing.
"""

from __future__ import annotations

import ast
import importlib
import inspect
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
MODULES = [
    "larmorx.sdcflows",
    "larmorx.niworkflows",
    "larmorx.ndimage",
    "larmorx.skimage",
    "larmorx.bids",
    "larmorx.pipelines.larmorprepx.fieldmaps",
    "larmorx.pipelines.larmorprepx.estimators",
]


def _rust_value(v: str) -> str:
    v = v.strip()
    return {"true": "True", "false": "False", "None": "None"}.get(v, v)


def rust_functions() -> dict[str, list[tuple[str, str | None]] | None]:
    """name -> [(parameter, default or None)] (None when no signature is declared)."""
    out: dict[str, list[tuple[str, str | None]] | None] = {}
    registered: set[str] = set()
    found: dict[str, tuple[str, str]] = {}
    for f in sorted((REPO / "crates/larmorx-py/src").rglob("*.rs")):
        lines = f.read_text(encoding="utf-8").splitlines()
        registered |= set(re.findall(r"wrap_pyfunction!\((\w+)", "\n".join(lines)))
        i = 0
        while i < len(lines):
            if lines[i].strip().startswith("#[pyfunction"):
                attrs = [lines[i]]
                j = i + 1
                while j < len(lines) and not re.match(
                    r"\s*(pub(\(crate\))?\s+)?fn\s+\w+", lines[j]
                ):
                    attrs.append(lines[j])
                    j += 1
                if j < len(lines):
                    name = re.match(r"\s*(?:pub(?:\(crate\))?\s+)?fn\s+(\w+)", lines[j]).group(1)
                    found[name] = (" ".join(attrs), str(f))
                i = j
            i += 1
    for name, (attrs, _) in found.items():
        rn = re.search(r'(?<![\w])name\s*=\s*"(\w+)"', attrs)
        pyname = rn.group(1) if rn else name
        sig = re.search(r"signature\s*=\s*\((.*?)\)\)\]", attrs, re.S)
        params = None
        if sig:
            params = []
            for part in _split(sig.group(1)):
                part = part.strip()
                if not part or part in ("*", "/"):
                    continue
                if "=" in part:
                    k, v = part.split("=", 1)
                    params.append((k.strip(), _rust_value(v)))
                else:
                    params.append((part.lstrip("*"), None))
        if name in registered:
            out[pyname] = params
    return out


def _split(s: str) -> list[str]:
    parts, depth, cur = [], 0, ""
    for ch in s:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append(cur)
            cur = ""
        else:
            cur += ch
    parts.append(cur)
    return parts


def stub_functions() -> dict[str, list[tuple[str, str | None]]]:
    tree = ast.parse((REPO / "python/larmorx/_core.pyi").read_text(encoding="utf-8"))
    out = {}
    for node in tree.body:
        if isinstance(node, ast.FunctionDef):
            a = node.args
            names = [x.arg for x in (*a.posonlyargs, *a.args)]
            defaults = [None] * (len(names) - len(a.defaults)) + [
                ast.unparse(d) for d in a.defaults
            ]
            params = list(zip(names, defaults, strict=True))
            params += [
                (x.arg, None if d is None else ast.unparse(d))
                for x, d in zip(a.kwonlyargs, a.kw_defaults, strict=True)
            ]
            out[node.name] = params
    return out


def check_stubs() -> int:
    rust, stubs = rust_functions(), stub_functions()
    bad = 0
    for name in sorted(set(rust) - set(stubs)):
        print(f"STUB missing: _core.{name}")
        bad += 1
    for name in sorted(set(stubs) - set(rust)):
        print(f"STUB without a function: _core.{name}")
        bad += 1
    compared = 0
    for name in sorted(set(rust) & set(stubs)):
        if rust[name] is None:
            continue
        compared += 1
        r = [(k, v) for k, v in rust[name] if k != "py"]
        s = stubs[name]
        if [k for k, _ in r] != [k for k, _ in s]:
            print(
                f"STUB parameters differ: _core.{name}: Rust {[k for k, _ in r]}, stub {[k for k, _ in s]}"
            )
            bad += 1
            continue
        for (k, rv), (_, sv) in zip(r, s, strict=True):
            if sv == "...":
                continue
            if rv is not None and sv is not None:
                try:
                    if float(rv) == float(sv):
                        continue
                except ValueError:
                    pass
            if (rv is None) != (sv is None) or (
                rv is not None and rv != sv and rv.strip('"') != str(sv).strip("'\"")
            ):
                print(f"STUB default differs: _core.{name}({k}): Rust {rv}, stub {sv}")
                bad += 1
    print(
        f"stubs: {len(rust)} registered functions, {len(stubs)} stubs, {compared} signatures compared, {bad} gaps"
    )
    return bad


def api_text() -> dict[str, str]:
    return {p.name: p.read_text(encoding="utf-8") for p in sorted((REPO / "docs/api").glob("*.md"))}


def _about(page: str, pat: re.Pattern) -> str:
    """The sections of ``page`` (between headings) that name the function."""
    sections, cur = [], []
    for line in page.splitlines():
        if line.startswith("#") and cur:
            sections.append("\n".join(cur))
            cur = []
        cur.append(line)
    sections.append("\n".join(cur))
    return "\n".join(sec for sec in sections if pat.search(sec))


def public_functions(modname: str) -> list[tuple[str, object]]:
    mod = importlib.import_module(modname)
    names = getattr(mod, "__all__", None) or [n for n in dir(mod) if not n.startswith("_")]
    out = []
    for n in names:
        obj = getattr(mod, n, None)
        if inspect.isfunction(obj) or inspect.isbuiltin(obj):
            out.append((n, obj))
    return out


def check_api() -> int:
    pages = api_text()
    bad = funcs = 0
    for modname in MODULES:
        for name, fn in public_functions(modname):
            funcs += 1
            pat = re.compile(rf"`(?:[\w.]*\.)?{re.escape(name)}(?:\(|`)")
            where = [p for p, t in pages.items() if pat.search(t)]
            if not where:
                print(f"API no page names {modname}.{name}")
                bad += 1
                continue
            try:
                params = [
                    p
                    for p in inspect.signature(fn).parameters
                    if p not in ("self", "args", "kwargs") and not p.startswith("_")
                ]
            except (TypeError, ValueError):
                continue
            text = "\n".join(_about(pages[p], pat) for p in where)
            missing = [p for p in params if not re.search(rf"\b{re.escape(p)}\b", text)]
            if missing:
                print(
                    f"API {modname}.{name} ({', '.join(where)}): parameters not named: {', '.join(missing)}"
                )
                bad += 1
    print(f"api: {funcs} public functions, {bad} gaps")
    return bad


GLUE = re.compile(
    r"(__init__\.py|_core\.pyi|/tests?\.rs|larmorx-py/src/|defects\.rs|/cli/mod\.rs|lib\.rs|mod\.rs)$|larmorx-py/src/"
)


def check_provenance() -> int:
    base = subprocess.run(
        ["git", "merge-base", "HEAD", "main"], capture_output=True, text=True, cwd=REPO, check=True
    ).stdout.strip()
    files = subprocess.run(["git", "diff", "--name-only", "--diff-filter=AM", base, "HEAD", "--", "crates", "python/larmorx"],
                           capture_output=True, text=True, cwd=REPO, check=True).stdout.split()  # fmt: skip
    prov = "\n".join(
        p.read_text(encoding="utf-8") for p in (REPO / "crates").glob("*/PROVENANCE.md")
    )
    bad = n = 0
    for f in files:
        if not f.endswith((".rs", ".py")) or GLUE.search(f) or f.startswith("crates-"):
            continue
        n += 1
        stem = Path(f).stem
        rel = f.removeprefix("python/larmorx/")
        parent = Path(f).parent.name
        keys = [rel, f, f"{stem}::", f"src/{stem}.rs", f"/{stem}.rs", f"`{stem}`", f"{stem}.py",
                f"math/{stem}", f"{parent}::{stem}", f"{parent}/{stem}"]  # fmt: skip
        if not any(k in prov for k in keys):
            print(f"PROVENANCE names nothing of {f}")
            bad += 1
    print(f"provenance: {n} source files the part added or changed, {bad} not named")
    return bad


def main() -> int:
    bad = check_stubs() + check_api() + check_provenance()
    print("ALL OK" if bad == 0 else f"GAPS {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
