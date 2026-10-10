# SPDX-License-Identifier: Apache-2.0
"""Golden tests: every platform reproduces larmorx's own Linux x86_64 output, bit for bit
(CLAUDE.md rule 10, PLAN.md §11.4).

The cases are in ``registry.py``, their checksums in ``golden.json`` (written on Linux x86_64
by ``scripts/record_golden.py``). On a mismatch the test names the outputs that differ, by
how much at the recorded sample positions, and saves the outputs in
``$LARMORX_GOLDEN_MISMATCHES`` (CI uploads that directory). See ``docs/validation/golden.md``.
"""

from __future__ import annotations

import importlib.util
import json
import os
import sys
from pathlib import Path

import pytest


def _load_registry():  # type: ignore[no-untyped-def]
    """``registry.py`` next to this file (the tests are not a package)."""
    name = "larmorx_golden_registry"
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name("registry.py"))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


registry = _load_registry()

HOW_TO = (
    "Add a case to tests/golden/registry.py naming them in `covers`, record the checksums on "
    "Linux x86_64 (python scripts/record_golden.py) and commit tests/golden/golden.json; see "
    "docs/validation/golden.md, 'Adding a golden case'."
)
#: The mismatching outputs saved at most (bytes), so the CI artifact stays small.
MAX_DUMP_BYTES = 50 << 20


@pytest.fixture(scope="session")
def golden() -> dict:
    with open(registry.GOLDEN_JSON, encoding="utf-8") as f:
        return json.load(f)


@pytest.fixture(scope="session")
def inputs(tmp_path_factory: pytest.TempPathFactory):  # type: ignore[no-untyped-def]
    return registry.Inputs(tmp_path_factory.mktemp("golden-inputs"))


def _dump_root(tmp_path: Path) -> Path:
    env = os.environ.get("LARMORX_GOLDEN_MISMATCHES")
    return Path(env) if env else tmp_path / "mismatches"


def _size(root: Path) -> int:
    return sum(p.stat().st_size for p in root.rglob("*") if p.is_file()) if root.exists() else 0


@pytest.mark.parametrize("case_id", sorted(registry.CASES))
def test_golden(case_id: str, golden: dict, inputs, tmp_path: Path) -> None:  # type: ignore[no-untyped-def]
    recorded = golden["cases"].get(case_id)
    if recorded is None:
        pytest.fail(f"{case_id} has no recorded checksums. {HOW_TO}")
    work = tmp_path / "work"
    work.mkdir()
    outputs = registry.CASES[case_id].run(registry.Context(work, inputs))
    actual = registry.fingerprints(outputs)
    problems = registry.compare(recorded, actual)
    if problems:
        root = _dump_root(tmp_path)
        saved = "not saved (the artifact is full)"
        if _size(root) < MAX_DUMP_BYTES:
            names = registry.mismatched(recorded, actual)
            saved = f"saved in {registry.dump(case_id, outputs, names, root)}"
        source = golden["recorded"]
        if os.environ.get("GITHUB_ACTIONS") == "true":
            # A workflow annotation: unlike the job log, GitHub's API serves annotations of a
            # public repository without a login, so the mismatches can be read from anywhere.
            detail = "; ".join(problems).replace("%", "%25").replace("\n", " ")
            sys.__stdout__.write(f"::error title=golden {case_id}::{detail[:900]}\n")
            sys.__stdout__.flush()
        pytest.fail(
            f"{case_id} differs from larmorx on Linux x86_64 (recorded from "
            f"{source['commit'][:12]} on {source['date']}):\n  "
            + "\n  ".join(problems)
            + f"\nOutputs {saved}; `python scripts/record_golden.py --compare <dir>` on Linux "
            "x86_64 measures the full difference."
        )


def test_every_tool_operation_and_function_has_a_case(golden: dict) -> None:
    """The CLI tools of every family, every ImageMath operation and every public function
    (``registry.NAMESPACES``) need a golden case, and ``golden.json`` must match the
    registry."""
    covered: set[str] = set()
    unknown = []
    for case in registry.CASES.values():
        for name in case.covers:
            try:
                covered.add(registry.covered_key(name))
            except (AttributeError, KeyError):
                unknown.append(f"{case.id}: {name}")
    missing = sorted(name for key, name in registry.required().items() if key not in covered)
    stale = sorted(set(golden["cases"]) - set(registry.CASES))
    messages = []
    if missing:
        messages.append(f"No golden case covers: {', '.join(missing)}. {HOW_TO}")
    if unknown:
        messages.append(f"`covers` names that do not exist: {', '.join(unknown)}")
    if stale:
        messages.append(
            f"golden.json has cases the registry no longer has: {', '.join(stale)}; re-record "
            "(python scripts/record_golden.py)."
        )
    assert not messages, "\n".join(messages)
