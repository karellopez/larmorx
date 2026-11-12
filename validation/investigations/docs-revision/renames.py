# SPDX-License-Identifier: Apache-2.0
"""The documentation revision's renames: every reference to a renamed file, changed or listed.

The writing guide (``docs/writing-guide.md``, section 1, "Renamed files") renames the files named
``*-original*`` that name a clean room: ``*-clean-room*`` (``*_clean_room*`` for Python modules).
This script changes every reference to them in the files that may change, and lists the
references in the files this part never edits (history, the pages made by a measurement, the
specifications, the clean rooms' own code), for which the renamed findings keep a short pointer
at their old path.

    python -P validation/investigations/docs-revision/renames.py --repo . --site <site checkout>
        [--apply] [--handoff <file.toml>] [--kept <file.md>]

Without ``--apply`` it changes nothing and prints what it would change, file by file. A shared
document that changes only through the orchestrator's handoff (``docs/next-steps.md``) gets
``[[op]]`` edits in ``--handoff`` (``scripts/orchestration/apply_handoff.py``'s format) instead.
``--kept`` writes the references left as they are, by who may change them.

What is renamed (26 files, the inventory's "renamed now", each record's ``.json`` kept beside its
``.md``): the clean rooms' own findings, the tests, and the validation package's modules. A record
(``docs/validation/<name>.json`` and ``.md``, one name read by one loader) and the benchmark
report's data that a page made by a measurement links to keep their names until those pages are
made again; so do the comparison suites' names (a suite's name is its record's), random testing's
descriptor names and cases, and the investigations' folders.
"""

from __future__ import annotations

import argparse
import re
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

_MODULES = (
    (
        "difftest/tools",
        ("afni_volreg", "fsl_avwutils", "fsl_fast", "fsl_prelude", "fsl_topup", "tedana_t2smap"),
    ),
    ("evidence", ("afni_volreg", "fsl_bet_options", "fsl_maths", "fsl_prelude", "tedana_t2smap")),
    ("parity", ("afni_volreg", "tedana_t2smap")),
)

#: (old path, new path), in this repository.
RENAMES = [
    *[
        (f"docs/findings/{s}-original.md", f"docs/findings/{s}-clean-room.md")
        for s in (
            "afni-masks",
            "afni-volreg",
            "fsl-bet",
            "fsl-fast",
            "fsl-flirt",
            "fsl-topup",
            "mri-hmc",
            "tedana-t2smap",
        )
    ],
    *[
        (f"tests/{d}/test_{s}_original.py", f"tests/{d}/test_{s}_clean_room.py")
        for d, s in (
            ("parity", "afni_volreg"),
            ("parity", "fsl_output_types"),
            ("parity", "tedana_t2smap"),
            ("python", "afni_masks"),
        )
    ],
    *[
        (
            f"validation/src/larmorx_validation/{pkg}/{m}_original.py",
            f"validation/src/larmorx_validation/{pkg}/{m}_clean_room.py",
        )
        for pkg, mods in _MODULES
        for m in mods
    ],
    (
        "validation/tests/test_evidence_tedana_t2smap_original.py",
        "validation/tests/test_evidence_tedana_t2smap_clean_room.py",
    ),
]

#: The findings renamed (their stems). afni-volreg's and tedana-t2smap's records have the same
#: stem (``docs/validation/afni-volreg-original.md``), so a bare name of these two outside
#: docs/findings/ is read by hand.
FINDINGS = [Path(old).stem[: -len("-original")] for old, _ in RENAMES if "/findings/" in old]
AMBIGUOUS = {"afni-volreg", "tedana-t2smap"}
#: The Python names renamed: a module's or a test file's stem without "_original" (``fsl_fast``).
MODULES = sorted(
    {
        Path(old).stem.removeprefix("test_").removeprefix("evidence_")[: -len("_original")]
        for old, _ in RENAMES
        if old.endswith("_original.py")
    },
    key=len,
    reverse=True,
)

# ------------------------------------------------------------------------------- which files
#: Files this part never edits (the first match wins): (pattern, who changes them).
KEPT = [
    (r"^docs/parts/|^docs/status-log\.md$|^docs/decision-log\.md$|^CHANGELOG\.md$", "history"),
    (r"^validation/(?:investigations|probes)/(?!docs-revision/)", "history"),
    (r"^validation/investigations/docs-revision/inventory\.md$", "generated: old_words.py"),
    (r"^specs/", "a spec writer"),
    (
        r"^crates/larmorx-(?:afni|mri|fsl|tedana)/|^crates/larmorx-core/src/math/"
        r"(?:cbrtf_glibc|sincos_glibc|sincos_glibc_tables|tanh_glibc)\.rs$",
        "a clean-room agent",
    ),
    (r"^docs/validation/clean-room-audit\.|^validation/cleanroom-audit/", "the orchestrator"),
    (
        r"^validation/src/larmorx_validation/cleanroom_audit/"
        r"|^validation/tests/test_cleanroom_audit\.py$",
        "the orchestrator (the audit accepts both names, and tests both)",
    ),
    (r"^CLAUDE\.md$", "the orchestrator"),
    (r"^docs/validation/[^/]+\.json$|^docs/benchmarks/", "made by a measurement"),
    (r"^docs/validation/README\.md$", "generated: tables (from validation/tools.toml)"),
    (r"^validation/difftest/cases/", "identifiers (random testing's cases)"),
    (r"^site:(?:static|data)/|^site:content/programs/", "generated: exported again"),
]
#: Shared documents that change through the orchestrator's handoff.
HANDOFF = {"docs/next-steps.md"}
#: A record's page names the command that wrote it (old_words.MADE_MARK's first form).
MADE = re.compile(
    r"^- \*\*Generated:\*\*(?:(?!\n\n|\n- ).)*?with `python -m larmorx_validation", re.M | re.S
)
TEXT = re.compile(r"\.(md|py|pyi|toml|rs|yml|yaml|txt|json|html|sh|cfg|ini|js|css)$")


def tracked(root: Path) -> list[str]:
    out = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True)
    return [p for p in out.stdout.splitlines() if TEXT.search(p)]


def who_keeps(key: str, text: str) -> str | None:
    for rx, who in KEPT:
        if re.search(rx, key):
            return who
    if re.match(r"^docs/validation/[^/]+\.md$", key) and MADE.search(text):
        return "made by a measurement"
    return None


# --------------------------------------------------------------------------------- the changes
def rewrite(rel: str, text: str) -> tuple[str, Counter, list[str]]:
    """The text with every reference changed, what changed, and the bare names read by hand."""
    n: Counter = Counter()
    review: list[str] = []
    in_findings = rel.startswith("docs/findings/")
    for stem in FINDINGS:
        new = f"{stem}-clean-room.md"
        text, k = re.subn(rf"(?<=findings/){re.escape(stem)}-original\.md", new, text)
        n[f"findings/{stem}"] += k
        bare = re.compile(rf"(?<![\w/.-]){re.escape(stem)}-original\.md")
        if in_findings or stem not in AMBIGUOUS:
            text, k = bare.subn(new, text)
            n[f"findings/{stem} (bare)"] += k
        else:
            for m in bare.finditer(text):
                line = text.count("\n", 0, m.start()) + 1
                review.append(f"{rel}:{line}: {m.group(0)}")
    for stem in MODULES:
        text, k = re.subn(rf"{re.escape(stem)}_original(?![a-z])", f"{stem}_clean_room", text)
        n[f"module {stem}"] += k
    return text, +n, review


def toml_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def handoff_ops(rel: str, before: str, after: str) -> list[str]:
    """One exact-replacement op per changed line (each such line is unique in its file)."""
    ops = []
    for old, new in zip(before.splitlines(), after.splitlines(), strict=True):
        if old != new:
            if before.count(old) != 1:
                raise SystemExit(f"{rel}: a changed line is not unique: {old[:80]!r}")
            ops.append(
                f'[[op]]\ndo = "edit"\nfile = "{rel}"\nold = {toml_str(old)}\nnew = {toml_str(new)}\n'
            )
    return ops


# ----------------------------------------------------------------------------------- main
def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--site", type=Path)
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--handoff", type=Path)
    ap.add_argument("--kept", type=Path)
    a = ap.parse_args()

    roots = [("", a.repo)] + ([("site:", a.site)] if a.site else [])
    changed: dict[str, Counter] = {}
    kept: dict[str, list[str]] = defaultdict(list)
    review: list[str] = []
    ops: list[str] = []
    any_old = re.compile(
        "|".join(
            [rf"(?:findings/|(?<![\w/.-])){re.escape(s)}-original\.md" for s in FINDINGS]
            + [rf"{re.escape(s)}_original(?![a-z])" for s in MODULES]
        )
    )
    for prefix, root in roots:
        for rel in tracked(root):
            try:
                text = (root / rel).read_text(encoding="utf-8")
            except (UnicodeDecodeError, FileNotFoundError):
                continue
            key = prefix + rel
            new, n, rv = rewrite(rel, text)
            if not n and not rv:
                continue
            who = who_keeps(key, text)
            if who is not None:
                for m in any_old.finditer(text):
                    line = text.count("\n", 0, m.start()) + 1
                    kept[who].append(f"{key}:{line}: {m.group(0)}")
                continue
            review += [prefix + r for r in rv]
            if not n:
                continue
            changed[key] = n
            if key in HANDOFF:
                ops += handoff_ops(rel, text, new)
            elif a.apply:
                (root / rel).write_text(new, encoding="utf-8")
    for key, n in sorted(changed.items()):
        how = "handoff" if key in HANDOFF else ("changed" if a.apply else "would change")
        print(f"{how}: {key}: " + ", ".join(f"{k} {v}" for k, v in sorted(n.items())))
    print(f"{len(changed)} files, {sum(sum(n.values()) for n in changed.values())} references")
    for r in review:
        print(f"read by hand: {r}")
    for who, refs in sorted(kept.items()):
        files = {r.rsplit(":", 2)[0] for r in refs}
        print(f"kept ({who}): {len(refs)} references in {len(files)} files")
    if a.handoff:
        a.handoff.write_text("\n".join(ops), encoding="utf-8")
        print(f"{len(ops)} handoff ops: {a.handoff}")
    if a.kept:
        lines = ["# The references to renamed files left as they are", ""]
        for who, refs in sorted(kept.items()):
            lines += [f"## {who} ({len(refs)})", ""] + [f"- `{r}`" for r in refs] + [""]
        a.kept.write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    main()
