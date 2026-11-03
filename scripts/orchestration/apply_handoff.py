# SPDX-License-Identifier: Apache-2.0
"""Apply an agent's machine-readable handoff (``HANDOFF.toml``) to the shared documents, all or nothing.

Usage: python -P apply_handoff.py <HANDOFF.toml> [--site <site checkout>] [--check]
       (run from the checkout: paths are relative to it; ``site:<path>`` names a file of --site)

The agent writes, beside its HANDOFF.md (the reasons, for the reader), the exact changes as a list of
TOML operations, applied in their order; every one is checked first, and nothing is written unless
all of them apply:

    [[op]]                # an exact replacement: `old` must occur exactly once in `file`
    do = "edit"
    file = "docs/findings/upstream-defects.md"
    old = "..."
    new = "..."

    [[op]]                # `text` inserted after the one line that starts with `after`
    do = "insert"
    file = "docs/api/upstream-defects.md"
    after = "| FSL-77 |"
    text = "..."

    [[op]]                # `text` appended at the end of `file`, after one blank line
    do = "append"
    file = "validation/difftest/triage.toml"
    text = "..."

    [[op]]                # replaces validation/tools.toml's [[tool]] or [[record]] block of that id
    do = "block"
    kind = "tool"
    id = "fsl-flirt"
    text = "..."          # the whole block, from its [[tool]] line

--check applies them in memory only and reports. Afterwards: `python -m larmorx_validation tables`,
`register_counts.py`, and re-read every file the handoff touched (docs/orchestration.md, merge step 6).
"""

import argparse
import json
import sys
import tomllib
from pathlib import Path


def target(path: str, repo: Path, site: Path | None) -> Path:
    if path.startswith("site:"):
        if site is None:
            raise SystemExit(f"{path}: a site file needs --site")
        root, rel = site, path[len("site:") :]
    else:
        root, rel = repo, path
    p = (root / rel).resolve()
    if root.resolve() not in p.parents:
        raise SystemExit(f"{path}: outside {root}")
    return p


def apply(op: str, spec: dict, text: str) -> str:
    if op == "edit":
        n = text.count(spec["old"])
        if n != 1:
            raise ValueError(f"`old` occurs {n} times (must be once): {spec['old'][:80]!r}")
        return text.replace(spec["old"], spec["new"])
    if op == "insert":
        lines = text.split("\n")
        hits = [i for i, line in enumerate(lines) if line.startswith(spec["after"])]
        if len(hits) != 1:
            raise ValueError(f"{len(hits)} lines start with {spec['after']!r} (must be one)")
        lines[hits[0] + 1 : hits[0] + 1] = spec["text"].rstrip("\n").split("\n")
        return "\n".join(lines)
    if op == "append":
        return text.rstrip("\n") + "\n\n" + spec["text"].rstrip("\n") + "\n"
    if op == "block":
        lines = text.split("\n")
        head = f"[[{spec['kind']}]]"
        idline = f'id = "{spec["id"]}"'
        hits = [i for i in range(1, len(lines)) if lines[i] == idline and lines[i - 1] == head]
        if len(hits) != 1:
            raise ValueError(f"{len(hits)} {head} blocks with {idline} (must be one)")
        a, b = hits[0] - 1, hits[0]
        while b + 1 < len(lines) and lines[b + 1].strip() and not lines[b + 1].startswith("[["):
            b += 1
        new = spec["text"].strip("\n").split("\n")
        if new[0] != head or idline not in new:
            raise ValueError(f"the new block must start with {head} and hold {idline}")
        lines[a : b + 1] = new
        return "\n".join(lines)
    raise ValueError(f"unknown operation {op!r}")


def parse(path: Path, text: str) -> None:
    """Parses a TOML, JSON or YAML file's new text (YAML when PyYAML is installed); raises if it fails."""
    if path.suffix == ".toml":
        tomllib.loads(text)
    elif path.suffix == ".json":
        json.loads(text)
    elif path.suffix in (".yml", ".yaml"):
        try:
            import yaml
        except ImportError:
            print(f"note  {path.name}: PyYAML not installed, not parsed")
            return
        yaml.safe_load(text)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("handoff")
    ap.add_argument("--site", type=Path, default=None)
    ap.add_argument("--check", action="store_true", help="apply in memory only")
    args = ap.parse_args()
    repo = Path.cwd()
    ops = tomllib.loads(Path(args.handoff).read_text(encoding="utf-8")).get("op", [])
    pending: dict[Path, str] = {}
    failed = 0
    for n, spec in enumerate(ops):
        op = spec.get("do", "?")
        name = "validation/tools.toml" if op == "block" else spec.get("file", "?")
        try:
            path = target(name, repo, args.site)
            text = pending.get(path)
            pending[path] = apply(
                op, spec, path.read_text(encoding="utf-8") if text is None else text
            )
            print(f"ok    [{n}] {op:6s} {name}")
        except (ValueError, KeyError, OSError) as e:
            failed += 1
            print(f"FAIL  [{n}] {op:6s} {name}: {e}")
    # every changed data file must still parse (2026-11-03: a status.yml value that began with a
    # backtick broke the site's build, and with it the Pages deploy)
    for path, text in pending.items():
        try:
            parse(path, text)
        except Exception as e:  # any parser's error
            failed += 1
            print(f"FAIL  {path.name} no longer parses: {e}")
    if failed:
        print(f"{failed} of {len(ops)} operation(s) do not apply: nothing written")
        return 1
    if args.check:
        print(f"{len(ops)} operation(s) apply to {len(pending)} file(s) (--check: nothing written)")
        return 0
    for path, text in pending.items():
        path.write_text(text, encoding="utf-8")
    print(f"{len(ops)} operation(s) written to {len(pending)} file(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
