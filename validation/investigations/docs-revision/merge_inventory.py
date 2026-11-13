# SPDX-License-Identifier: Apache-2.0
"""The merge's judgement of the site's before/after inventory (the documentation revision, step 1, relay 28).

``site_inventory.py`` (validation/investigations/difftest/) marks a page NO when a shown number's pointer
changed, which also happens when a number only moved: a table's rows reordered, a list's entries shifted by a
new one, the default view showing another comparison of the same value. Three checks tell the moves apart from
a loss, each against a build of the published site (with the moved pages paired at their new addresses):

    python -P merge_inventory.py shown <before site> <after site>
        per page, every shown number's text as a multiset: the texts the page shows fewer times after
        (a value no longer shown, or shown less often); nothing but the records made again may appear
    python -P merge_inventory.py anchors <before site, unpaired> <after site>
        every anchor (id) of every old page that is now a redirect: does the reader land on it (the redirect's
        address, or the page its ``anchors:`` map names)?
    python -P merge_inventory.py json <before evidence JSON> <after evidence JSON>
        one evidence file, its lists matched by identity (id, entry, category, page with /tools/ read as
        /evidence/, ...) or, for rows without one, by their numbers: every number and field of before that
        after no longer holds, by path; texts counted, not listed (the revision rewrites them)

Each site is a checkout of larmorx_documentation built with ``tools/build.py`` (``_site/``).
"""

from __future__ import annotations

import collections
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "difftest"))
from site_inventory import parse


def shown(before: Path, after: Path) -> int:
    pages = sorted(
        str(p.parent.relative_to(after / "_site")) for p in (after / "_site").rglob("index.html")
    )
    total = 0
    for page in pages:
        fb = before / "_site" / page / "index.html"
        if not fb.is_file():
            continue
        pb, pa = parse(fb), parse(after / "_site" / page / "index.html")
        cb = collections.Counter(t for v in pb.shown.values() for t in v)
        ca = collections.Counter(t for v in pa.shown.values() for t in v)
        if not cb:
            continue
        less = cb - ca
        total += sum(less.values())
        print(
            f"{page}: {sum(cb.values())} -> {sum(ca.values())} shown; "
            f"texts shown less after: {sum(less.values())}" + (f" {dict(less)}" if less else "")
        )
    print("TOTAL texts shown less:", total)
    return 0


def _ids(p: Path) -> set[str]:
    return set(re.findall(r'\sid="([^"]+)"', p.read_text(encoding="utf-8")))


def _front(md: Path) -> dict:
    import yaml

    t = md.read_text(encoding="utf-8")
    return (yaml.safe_load(t.split("---", 2)[1]) or {}) if t.startswith("---") else {}


def anchors(before: Path, after: Path) -> int:
    def page(site: Path, url: str) -> tuple[Path, str]:
        path, _, frag = url.partition("#")
        return site / "_site" / path.strip("/") / "index.html", frag

    chrome = _ids(before / "_site" / "index.html") & _ids(
        before / "_site" / "roadmap" / "index.html"
    )
    bad = total = 0
    for md in sorted((after / "content").rglob("*.md")):
        meta = _front(md)
        if not meta.get("redirect"):
            continue
        rel = md.relative_to(after / "content").with_suffix("")
        if rel.name in ("index", "_index"):
            rel = rel.parent
        old = before / "_site" / rel / "index.html"
        if not old.is_file():
            print(f"{rel}: no old page in the before build")
            continue
        land_ids = _ids(page(after, meta["redirect"])[0])
        moved = meta.get("anchors") or {}
        missing = []
        for i in sorted(_ids(old) - chrome):
            total += 1
            if i in moved:
                f, frag = page(after, moved[i])
                if frag and frag not in _ids(f):
                    missing.append(f"{i} -> {moved[i]} (missing there)")
            elif i not in land_ids:
                missing.append(i)
        bad += len(missing)
        print(
            f"{rel} -> {meta['redirect']}: {len(_ids(old) - chrome)} anchors, "
            f"{len(missing)} not landing" + (f": {missing}" if missing else "")
        )
    print(f"TOTAL anchors {total}, not landing {bad}")
    return 1 if bad else 0


KEYS = ("id", "entry", "category", "page", "tool", "name", "key")


def _norm(k):
    return k.replace("/tools/", "/evidence/") if isinstance(k, str) else k


def _numbers(x):
    if isinstance(x, dict):
        return tuple((k, _numbers(v)) for k, v in sorted(x.items()) if not isinstance(v, str))
    if isinstance(x, list):
        return tuple(_numbers(v) for v in x if not isinstance(v, str))
    return x


def json_diff(a_path: Path, b_path: Path) -> int:
    counts: collections.Counter[str] = collections.Counter()
    notes: list[str] = []

    def by_key(x: list, y: list, p: str) -> bool:
        for key in KEYS:
            kx = [str(_norm(e.get(key))) for e in x]
            ky = [str(_norm(e.get(key))) for e in y]
            if "None" in kx or "None" in ky or len(set(kx)) < len(kx) or len(set(ky)) < len(ky):
                continue
            iy = {k: i for i, k in enumerate(ky)}
            for i, e in enumerate(x):
                j = iy.get(kx[i])
                if j is None:
                    notes.append(f"element gone {p}[{key}={e[key]}]")
                    continue
                if i != j:
                    counts["moved element"] += 1
                walk(e, y[j], f"{p}[{key}={e[key]}]")
            counts["added element"] += len(set(ky) - set(kx))
            return True
        return False

    def by_numbers(x: list, y: list, p: str) -> None:
        sig = collections.defaultdict(list)
        for j, e in enumerate(y):
            sig[_numbers(e)].append(j)
        used: set[int] = set()
        unmatched = []
        for i, e in enumerate(x):
            js = [j for j in sig.get(_numbers(e), []) if j not in used]
            if js:
                used.add(js[0])
                if js[0] != i:
                    counts["moved row"] += 1
                walk(e, y[js[0]], f"{p}/{i}->{js[0]}")
            else:
                unmatched.append(i)
        free = [j for j in range(len(y)) if j not in used]
        for i in unmatched:
            if i in free:
                walk(x[i], y[i], f"{p}/{i}")
                free.remove(i)
            else:
                notes.append(f"row gone {p}/{i}: {json.dumps(_numbers(x[i]))[:200]}")
        counts["added element"] += max(0, len(y) - len(x))

    def walk(x, y, p: str) -> None:
        if isinstance(x, dict) and isinstance(y, dict):
            notes.extend(f"field gone {p}/{k}" for k in x.keys() - y.keys())
            for k in y.keys() - x.keys():
                counts["added " + k] += 1
            for k in x.keys() & y.keys():
                walk(x[k], y[k], f"{p}/{k}")
        elif isinstance(x, list) and isinstance(y, list):
            if x and y and all(isinstance(e, dict) for e in x + y):
                if not by_key(x, y, p):
                    by_numbers(x, y, p)
                return
            if len(y) < len(x):
                notes.append(f"shorter {p}: {len(x)} -> {len(y)}")
            elif len(y) > len(x):
                counts["longer list"] += 1
            for i, (u, v) in enumerate(zip(x, y, strict=False)):
                walk(u, v, f"{p}/{i}")
        elif isinstance(x, str) and isinstance(y, str):
            if x != y:
                counts["text changed"] += 1
        elif x != y:
            notes.append(f"value {p}: {x!r} -> {y!r}")

    a, b = (json.loads(f.read_text(encoding="utf-8")) for f in (a_path, b_path))
    walk(a, b, "")
    print(json.dumps(dict(counts)))
    for n in notes:
        print(n)
    return 0


if __name__ == "__main__":
    cmd, x, y = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
    sys.exit({"shown": shown, "anchors": anchors, "json": json_diff}[cmd](x, y))
