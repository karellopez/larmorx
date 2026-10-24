# SPDX-License-Identifier: Apache-2.0
"""``python -m larmorx_validation oracle-cache <command>``.

- ``verify --sample N``: re-run N cached entries chosen at random and check that they give the
  same result (the drift detector: a changed oracle changes the key, this catches what the key
  misses). Exit status 1 when one differs: a finding, kept in ``disputes/`` and
  ``findings.jsonl``, never overwritten.
- ``confirm``: re-run the entries above one thread that are not yet served (they need
  ``CONFIRMATIONS`` identical runs), until they are or one differs.
- ``stats``: what the cache holds.
- ``disputes``: the entries a run disagreed with.
"""

from __future__ import annotations

import argparse
import random
import sys
from collections import Counter
from pathlib import Path

from larmorx_validation.oracle_cache.rerun import Outcome, rerun_many
from larmorx_validation.oracle_cache.store import Store


def _store(args: argparse.Namespace) -> Store:
    return Store(Path(args.cache) if args.cache else None)


def _complete(store: Store) -> list[dict]:
    return [
        e
        for e in store.entries()
        if not e.get("disputed") and e.get("confirmations", 0) >= e.get("required", 1)
    ]


def _print(o: Outcome) -> None:
    mark = {"same": "same", "differs": "DIFFERS", "skipped": "skipped"}[o.status]
    text = f"{mark:8s} {o.key[:16]} {o.seconds:8.1f} s"
    print(text + (f"  {o.detail}" if o.detail else ""), flush=True)


def cmd_verify(args: argparse.Namespace) -> int:
    store = _store(args)
    entries = _complete(store)
    if args.program:
        entries = [e for e in entries if e["parts"]["oracle"].get("program") in args.program]
    rng = random.Random(args.seed)
    chosen = rng.sample(entries, min(args.sample, len(entries)))
    print(f"oracle cache {store.root}: {len(entries)} entries served; re-running {len(chosen)}")
    outcomes = rerun_many(
        store, [e["key"] for e in chosen], jobs=args.jobs, timeout=args.timeout, progress=_print
    )
    counts = Counter(o.status for o in outcomes)
    print(", ".join(f"{v} {k}" for k, v in sorted(counts.items())) or "nothing to verify")
    if counts["differs"]:
        print(
            f"VERIFY FOUND {counts['differs']} DIFFERENCE(S): the entries are disputed (never "
            f"served again) and kept with the new results in {store.root / 'disputes'}; see "
            f"{store.root / 'findings.jsonl'}",
            file=sys.stderr,
        )
        return 1
    return 0


def cmd_confirm(args: argparse.Namespace) -> int:
    store = _store(args)
    pending = [
        e
        for e in store.entries()
        if not e.get("disputed") and e.get("confirmations", 0) < e.get("required", 1)
    ]
    if args.program:
        pending = [e for e in pending if e["parts"]["oracle"].get("program") in args.program]
    pending = pending[: args.max] if args.max else pending
    keys = [k for e in pending for k in [e["key"]] * (e["required"] - e.get("confirmations", 0))]
    print(f"oracle cache {store.root}: {len(pending)} entries to confirm ({len(keys)} runs)")
    # Each entry's runs one after the other (a run that differs stops its entry's next ones).
    by_entry: dict[str, int] = Counter(keys)
    order = sorted(by_entry)
    outcomes: list[Outcome] = []
    for _ in range(max(by_entry.values(), default=0)):
        batch = [k for k in order if by_entry[k] > 0]
        for o in rerun_many(store, batch, jobs=args.jobs, timeout=args.timeout, progress=_print):
            outcomes.append(o)
            by_entry[o.key] = by_entry[o.key] - 1 if o.status == "same" else 0
    counts = Counter(o.status for o in outcomes)
    print(", ".join(f"{v} {k}" for k, v in sorted(counts.items())) or "nothing to confirm")
    return 1 if counts["differs"] else 0


def cmd_stats(args: argparse.Namespace) -> int:
    store = _store(args)
    entries = list(store.entries())
    by_program: Counter = Counter(e["parts"]["oracle"].get("program") for e in entries)
    served = len(_complete(store))
    pending = sum(
        1 for e in entries if not e.get("disputed") and e.get("confirmations", 0) < e["required"]
    )
    disputed = sum(1 for e in entries if e.get("disputed"))
    size = sum(p.stat().st_size for p in (store.root / "blobs").glob("*/*")) if entries else 0
    seconds = sum(e.get("seconds", 0.0) for e in entries)
    print(f"oracle cache {store.root}")
    print(
        f"{len(entries)} entries: {served} served, {pending} awaiting confirmation, "
        f"{disputed} disputed; blobs {size / 2**30:.2f} GiB; the original runs took "
        f"{seconds / 3600:.2f} h"
    )
    for program, n in by_program.most_common():
        print(f"  {n:6d}  {program}")
    return 0


def cmd_disputes(args: argparse.Namespace) -> int:
    store = _store(args)
    found = [e for e in store.entries() if e.get("disputed")]
    for e in found:
        d = e["disputed"]
        print(f"{e['key'][:16]} {e['parts']['oracle'].get('program')} ({e.get('case', '')})")
        print(f"    {e.get('command', '')[:200]}")
        for line in d.get("differences", []):
            print(f"    - {line}")
        print(f"    record: {d.get('record')}")
    print(f"{len(found)} disputed entries")
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--cache", help="the cache (default: LARMORX_ORACLE_CACHE, else <workspace>/oracles/cache)"
    )
    sub = parser.add_subparsers(dest="oracle_command", required=True)
    v = sub.add_parser("verify", help="re-run random cached entries; any difference is a finding")
    v.add_argument("--sample", type=int, default=10)
    v.add_argument("--seed", type=int, default=None, help="the sample's seed (default: random)")
    v.add_argument("--program", nargs="+", help="only these programs (their names)")
    v.add_argument("--jobs", type=int, default=1)
    v.add_argument("--timeout", type=float, default=4 * 3600)
    v.set_defaults(oracle_func=cmd_verify)
    c = sub.add_parser("confirm", help="run the confirmations runs above one thread still need")
    c.add_argument("--max", type=int, default=0, help="at most this many entries")
    c.add_argument("--program", nargs="+", help="only these programs (their names)")
    c.add_argument("--jobs", type=int, default=1)
    c.add_argument("--timeout", type=float, default=4 * 3600)
    c.set_defaults(oracle_func=cmd_confirm)
    s = sub.add_parser("stats", help="what the cache holds")
    s.set_defaults(oracle_func=cmd_stats)
    d = sub.add_parser("disputes", help="the entries a run disagreed with")
    d.set_defaults(oracle_func=cmd_disputes)


def main(args: argparse.Namespace) -> int:
    return args.oracle_func(args)


__all__ = ["add_arguments", "main"]
