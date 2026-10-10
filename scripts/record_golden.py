# SPDX-License-Identifier: Apache-2.0
"""Record the golden checksums of the larmorx package (tests/golden/golden.json).

Golden tests check that every platform reproduces larmorx's own Linux x86_64 output
(CLAUDE.md rule 10, docs/validation/golden.md). The checksums are recorded here, on Linux
x86_64 only, from a committed tree whose parity with the original tools passes:

    python scripts/record_golden.py                  # run tests/parity (smoke tier), then record
    python scripts/record_golden.py --skip-parity    # record without parity (noted in the JSON)
    python scripts/record_golden.py --check          # compare with golden.json; write nothing
    python scripts/record_golden.py --compare DIR    # full differences of a CI artifact

``--compare`` takes the ``golden-mismatches-<platform>`` artifact of a failed CI job (the
outputs of the mismatching cases), recomputes those cases here and prints, for every output,
how many values differ, the largest absolute difference and the largest ulp distance.

Build the extension first (``maturin develop --release``); the script refuses to run with a
``larmorx`` imported from anywhere but this tree's ``python/``.
"""

from __future__ import annotations

import argparse
import datetime
import importlib.util
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
GOLDEN = ROOT / "tests" / "golden" / "golden.json"


def load_registry():  # type: ignore[no-untyped-def]
    name = "larmorx_golden_registry"
    spec = importlib.util.spec_from_file_location(name, ROOT / "tests" / "golden" / "registry.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=True, capture_output=True, text=True, encoding="utf-8"
    ).stdout.strip()


def check_environment() -> None:
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        sys.exit(
            f"golden checksums are recorded on Linux x86_64 only, not on {platform.system()} "
            f"{platform.machine()} (CLAUDE.md rule 10)"
        )
    import larmorx

    where = Path(larmorx.__file__).resolve()
    if not where.is_relative_to(ROOT / "python"):
        sys.exit(f"larmorx is imported from {where}, not from this tree; run maturin develop")


def run_cases(registry, ids: list[str] | None = None) -> tuple[dict, dict, dict]:  # type: ignore[no-untyped-def]
    """Run the cases: ``(fingerprints, raw outputs, seconds)`` by case id."""
    prints, outputs, seconds = {}, {}, {}
    with tempfile.TemporaryDirectory(prefix="larmorx-golden-") as tmp:
        inputs = registry.Inputs(Path(tmp) / "inputs")
        inputs.root.mkdir()
        for case_id in ids or sorted(registry.CASES):
            work = Path(tmp) / registry.safe_name(case_id)
            work.mkdir()
            start = time.perf_counter()
            out = registry.CASES[case_id].run(registry.Context(work, inputs))
            outputs[case_id] = registry.expand(out)
            seconds[case_id] = time.perf_counter() - start
            prints[case_id] = registry.fingerprints(out)
    return prints, outputs, seconds


def _one(value: object) -> str:
    return json.dumps(value, sort_keys=True, ensure_ascii=False)


def dumps(document: dict) -> str:
    """The JSON with one line per output, so that re-recording gives a readable diff."""
    cases = []
    for case_id, outputs in sorted(document["cases"].items()):
        lines = [f"   {_one(name)}: {_one(fp)}" for name, fp in sorted(outputs.items())]
        cases.append(f"  {_one(case_id)}: {{\n" + ",\n".join(lines) + "\n  }")
    return (
        "{\n"
        f' "about": {_one(document["about"])},\n'
        f' "recorded": {_one(document["recorded"])},\n'
        ' "cases": {\n' + ",\n".join(cases) + "\n }\n}\n"
    )


def record(registry, args: argparse.Namespace) -> None:  # type: ignore[no-untyped-def]
    dirty = [
        line
        for line in git("status", "--porcelain").splitlines()
        if not line.endswith("tests/golden/golden.json")
    ]
    if dirty and not args.allow_dirty:
        sys.exit(
            "the tree has uncommitted changes; commit them first (the checksums name the commit "
            "they come from), or pass --allow-dirty:\n  " + "\n  ".join(dirty)
        )
    if args.skip_parity:
        parity = "skipped"
    else:
        print("Running the parity suite (tests/parity, smoke tier) ...", flush=True)
        result = subprocess.run([sys.executable, "-m", "pytest", "tests/parity", "-q"], cwd=ROOT)
        if result.returncode != 0:
            sys.exit("parity failed: golden checksums are recorded only from a passing tree")
        parity = "passed (pytest tests/parity, smoke tier)"
    prints, _, seconds = run_cases(registry)
    commit = git("rev-parse", "HEAD")
    document = {
        "about": (
            "SHA-256 of larmorx's outputs on Linux x86_64 for the golden cases in registry.py; "
            "every platform must reproduce them (docs/validation/golden.md). Written by "
            "scripts/record_golden.py; do not edit by hand."
        ),
        "recorded": {
            "commit": commit + ("+dirty" if dirty else ""),
            "date": datetime.datetime.now(datetime.UTC).date().isoformat(),
            "platform": f"{platform.system()} {platform.machine()}",
            "python": platform.python_version(),
            "numpy": np.__version__,
            "parity": parity,
        },
        "cases": prints,
    }
    text = dumps(document)
    GOLDEN.write_text(text, encoding="utf-8")
    n_out = sum(len(p) for p in prints.values())
    print(
        f"Recorded {len(prints)} cases ({n_out} outputs) from {commit[:12]} in {GOLDEN} "
        f"({len(text.encode()) // 1024} KiB); the cases took {sum(seconds.values()):.2f} s."
    )
    for case_id, s in sorted(seconds.items(), key=lambda kv: -kv[1])[:5]:
        print(f"  {s * 1000:8.1f} ms  {case_id}")


def check(registry) -> int:  # type: ignore[no-untyped-def]
    golden = json.loads(GOLDEN.read_text(encoding="utf-8"))
    prints, _, seconds = run_cases(registry)
    bad = 0
    for case_id, actual in prints.items():
        problems = registry.compare(golden["cases"].get(case_id, {}), actual)
        if problems:
            bad += 1
            print(f"{case_id}:\n  " + "\n  ".join(problems))
    print(f"{len(prints) - bad} of {len(prints)} cases agree; {sum(seconds.values()):.2f} s")
    return 1 if bad else 0


def full_difference(expected: np.ndarray, actual: np.ndarray, registry) -> str:  # type: ignore[no-untyped-def]
    if expected.dtype != actual.dtype or expected.shape != actual.shape:
        return f"{expected.dtype} {expected.shape} here, {actual.dtype} {actual.shape} there"
    e = np.ascontiguousarray(expected).reshape(-1)
    a = np.ascontiguousarray(actual).reshape(-1)
    differ = e.view(np.uint8).reshape(e.size, -1) != a.view(np.uint8).reshape(a.size, -1)
    n = int(differ.any(axis=1).sum())
    if n == 0:
        return "identical"
    text = f"{n} of {e.size} values differ"
    if e.dtype.kind in "fiu":
        with np.errstate(all="ignore"):
            diff = np.abs(e.astype(np.float64) - a.astype(np.float64))
        text += f", max |difference| {np.nanmax(diff):.6g}"
    if e.dtype.kind == "f":
        text += f", max {registry.ulp_distance(e, a)} ulp"
    return text


def compare_artifact(registry, directory: Path) -> None:  # type: ignore[no-untyped-def]
    names = {registry.safe_name(c): c for c in registry.CASES}
    found = [d for d in sorted(directory.iterdir()) if d.is_dir() and d.name in names]
    if not found:
        sys.exit(f"{directory} holds no golden case outputs")
    _, outputs, _ = run_cases(registry, [names[d.name] for d in found])
    for d in found:
        case_id = names[d.name]
        print(case_id)
        for name, here in sorted(outputs[case_id].items()):
            stem = d / registry.safe_name(name)
            if isinstance(here, bytes):
                there = Path(f"{stem}.bin")
                if there.exists():
                    same = there.read_bytes() == here
                    print(
                        f"  {name}: {'identical' if same else 'differs'} ({len(here)} bytes here)"
                    )
            elif (npy := Path(f"{stem}.npy")).exists():
                print(f"  {name}: {full_difference(np.asarray(here), np.load(npy), registry)}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--skip-parity", action="store_true", help="record without running parity")
    parser.add_argument(
        "--allow-dirty", action="store_true", help="record from uncommitted changes"
    )
    parser.add_argument("--check", action="store_true", help="compare with golden.json only")
    parser.add_argument(
        "--compare", type=Path, metavar="DIR", help="full differences of a CI artifact"
    )
    args = parser.parse_args()
    check_environment()
    os.chdir(ROOT)
    registry = load_registry()
    if args.compare:
        compare_artifact(registry, args.compare)
        return 0
    if args.check:
        return check(registry)
    record(registry, args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
