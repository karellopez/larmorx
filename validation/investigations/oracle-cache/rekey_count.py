# SPDX-License-Identifier: Apache-2.0
"""How many entries of an oracle cache the corrections of 2026-10-28 re-key or retire, without
running anything: the entries whose suites' next runs compute another key (the new entry is
stored then; the old one stays, never reached) and those the current rules never cache.

Usage::

    python -P rekey_count.py [--cache <cache>] [--list]

What changed (``docs/validation/oracle-cache.md``, "Keyed since 2026-10-28"), and how an entry
is recognised here:

- **path text** (``policy.path_text``): a path the key replaces that holds whitespace (every
  program), or bet2 with one argument holding ``-h`` or ``--help``: the command as it ran
  (``command``) is compared with the key's arguments;
- **a path with a blank** taken apart before: a place's path followed by a blank in the
  arguments (``rerun.stale``);
- **files named by a pattern** (``prior%02d.nii.gz``): an argument with a printf pattern in a
  place (``rerun.stale``; the files existed when the run was stored if it did not fail on them);
- **the working directory's files** (``fsl-mcflirt-replica`` since 2026-10-28): the entries of
  ``rerun.SUPERSEDED``;
- **never cached now** (``policy.ants_never``): ANTS-82's stages, ANTS-07's smaller operand,
  ANTS-28's thin smoothed axis, evaluated on the entry's stored inputs.
"""

from __future__ import annotations

import argparse
import shlex
import sys
import tempfile
from collections import Counter
from pathlib import Path

from larmorx_validation.oracle_cache import policy, rerun
from larmorx_validation.oracle_cache.store import Store

_PH = rerun._PLACEHOLDER


def _args_as_run(entry: dict) -> list[str] | None:
    """The program's arguments as the stored command ran them (``~`` expanded)."""
    try:
        words = shlex.split(entry.get("command", ""))
    except ValueError:
        return None
    home = str(Path.home())
    words = [home + w[1:] if w.startswith("~") else w for w in words]
    n = len(entry.get("template", []))
    return words[-n:] if n else []


def path_text(entry: dict) -> str:
    template = entry.get("template", [])
    ran = _args_as_run(entry)
    if ran is None or len(ran) != len(template):
        return ""
    program = str(entry["parts"]["oracle"].get("program", ""))
    texts = policy.path_text(program, ran)
    for t, a in zip(template, ran, strict=True):
        if not _PH.search(t):
            continue
        # the text the key replaced: what the argument holds beyond the template's own text
        kept = _PH.sub("", t)
        found = [x for x in texts if x in a and x not in kept]
        if found:
            return f"a path the key replaces holds {found}"
    return ""


def never_now(store: Store, entry: dict) -> str:
    if entry.get("policy", {}).get("family") != "itk":
        return ""
    with tempfile.TemporaryDirectory(prefix="lx-rekey_") as t:
        values = {}
        for i, item in enumerate(entry.get("inputs", [])):
            link = Path(t) / f"in{i}{item['suffix']}"
            blob = store.blob_path(item["sha256"])
            if blob.exists():
                link.symlink_to(blob)
            values[f"in{i}"] = str(link)
        args = [_PH.sub(lambda m: values.get(m.group(1), m.group(0)), a) for a in entry["template"]]
        env = dict(entry.get("rerun_env", {}))
        program = Path(str(entry["parts"]["oracle"].get("program", ""))).name
        return policy.ants_never(program, args, env)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--cache", type=Path, default=None)
    ap.add_argument("--list", action="store_true", help="print every entry counted")
    args = ap.parse_args(argv)
    store = Store(args.cache)
    counts: Counter = Counter()
    programs: Counter = Counter()
    total = 0
    for e in store.entries():
        total += 1
        kind = ""
        if w := rerun.stale(e):
            kind = "re-keyed: " + w.split(" (")[0]
        elif w := path_text(e):
            kind = "re-keyed: path text"
        elif w := never_now(store, e):
            kind = "never cached now: " + w.split(":")[0]
        if kind:
            counts[kind] += 1
            programs[(kind, Path(str(e["parts"]["oracle"].get("program"))).name)] += 1
            if args.list:
                print(f"{e['key'][:16]} {kind:60s} {e.get('case', '')[:70]}")
    print(f"{total} entries in {store.root}")
    for kind, n in sorted(counts.items()):
        progs = ", ".join(f"{p} {m}" for (k, p), m in sorted(programs.items()) if k == kind)
        print(f"  {n:5d}  {kind}  ({progs})")
    print(f"  {total - sum(counts.values()):5d}  keep their keys")
    return 0


if __name__ == "__main__":
    sys.exit(main())
