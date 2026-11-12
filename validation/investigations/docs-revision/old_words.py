# SPDX-License-Identifier: Apache-2.0
"""The old words: where the documentation revision has to change them.

Counts, file by file, in this repository and in a checkout of the documentation site, the words
`docs/writing-guide.md` replaces: "clean-room original", "original" where it names a clean room,
"exact replica", "port" as a third kind beside the replica, the slogans and set phrases, and, on
the pages users read, the developers' jargon (oracle, parity, golden, tier, ...). Every other
"original" is counted too, as words to read: most name the program LARMORX is compared with,
which is right, but the rollout reads each one.

    python -P validation/investigations/docs-revision/old_words.py \\
        --repo . --site <site checkout> --out validation/investigations/docs-revision/inventory.md

Only tracked files are read (``git ls-files``); FSL's original source, licence texts, lock files,
fonts and the evidence's images are left out. Each file belongs to one area, and each area says
how its words change (rewritten in the rollout, history kept as written, a clean-room agent, a
spec writer, generated and re-made).
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

# --------------------------------------------------------------------------------------- words
FLAGS = re.IGNORECASE

#: "clean-room original(s)", also "clean room original" and "clean-room **original**".
CLEAN_ROOM_ORIGINAL = re.compile(r"clean[\s-]*room[\s*_`-]{0,4}originals?\b", FLAGS)

#: Forms in which "original" can only mean a clean room.
ORIGINAL_AS_CLEAN_ROOM = [
    # the value of the implementation option (still accepted, for 0.1.0rc1's users)
    re.compile(r"implementation['\"]?\s*[=:]\s*\\?['\"]original\\?['\"]", FLAGS),
    re.compile(r"--implementation[\s=]+['\"]?original\b", FLAGS),
    re.compile(r"LARMORX_IMPLEMENTATION\s*=\s*['\"]?original\b", FLAGS),
    # a difference's label: "original − FSL 6.0.7.23", "original (reproduce) − AFNI"
    re.compile(r"\boriginal\s*(?:\((?:default|reproduce)[^)]*\)\s*)?[−–]\s", FLAGS),
    # a status beside the replica's: "experimental (original)"
    re.compile(r"\(original\)", FLAGS),
    # a record belongs to a port, never to the program compared with
    re.compile(r"\boriginal's (?:[\w-]+[ -]tier )?records?\b", FLAGS),
    # "LARMORX's original", "larmorx original"
    re.compile(r"\blarmorx(?:'s)?\s+originals?\b", FLAGS),
    # CLAUDE.md's rule 1
    re.compile(r"\*\*Original:\*\*"),
]

#: Every "original(s)"; those not counted above are words to read.
ANY_ORIGINAL = re.compile(r"\boriginals?\b", FLAGS)

#: "exact replica", "bit-exact replica": the word replica already says it.
EXACT_REPLICA = re.compile(r"\b(?:bit-)?exact\s+replicas?\b", FLAGS)

#: "port" as a kind of its own beside the replica (ANTs "ported from the source").
PORT_AS_KIND = re.compile(
    r"ported from (?:the |their |its )?source|\ba port of (?:the |its )?source|"
    r"\b(?:two|three) ways of reimplementing\b",
    FLAGS,
)

#: Slogans and set phrases that go.
SLOGANS = [
    re.compile(r"many toolkits", FLAGS),
    re.compile(r"\bone\*? framework\b", FLAGS),
    re.compile(r"common computational framework", FLAGS),
    re.compile(r"clean-room development practices", FLAGS),
    re.compile(r"established (?:neuroimaging )?(?:tools|algorithms)", FLAGS),
    re.compile(r"each keeps its colour", FLAGS),
]

#: The tagline (the user's, 2026-10-19): counted apart; the guide asks whether it stays.
TAGLINE = re.compile(r"into \*?resonance", FLAGS)

#: Developers' words on the pages users read: say what the thing is instead.
JARGON = {
    "oracle": re.compile(r"\boracles?\b", FLAGS),
    "parity": re.compile(r"\bparity\b", FLAGS),
    "golden": re.compile(r"\bgolden\b", FLAGS),
    "tier": re.compile(r"\btiers?\b", FLAGS),
    "night": re.compile(r"\bnights?\b", FLAGS),
    "D13": re.compile(r"\bD13\b"),
    "upstream": re.compile(r"\bupstreams?\b", FLAGS),
    "ecosystem": re.compile(r"\becosystems?\b", FLAGS),
}

# ---------------------------------------------------------------------------------------- files
SKIP = re.compile(
    r"^validation/investigations/docs-revision/|^docs/writing-guide\.md$|(^|/)(fsl-source|LICENSES|fonts|blobs)/|Cargo\.lock$|\.lock$|requirements-lock\.txt$|"
    r"(^|/)LICENSE[^/]*$|\.(png|jpe?g|webp|gif|ico|svg|woff2?|ttf|otf|bin|gz|h5|nii|mat|pdf)$",
)
TEXT_LIMIT = 40 * 1024 * 1024

CLEAN_ROOM_CRATES = (
    "crates/larmorx-afni/",
    "crates/larmorx-mri/",
    "crates/larmorx-fsl/",
    "crates/larmorx-tedana/",
)
CLEAN_ROOM_CORE = (
    "crates/larmorx-core/src/math/cbrtf_glibc.rs",
    "crates/larmorx-core/src/math/sincos_glibc.rs",
    "crates/larmorx-core/src/math/sincos_glibc_tables.rs",
    "crates/larmorx-core/src/math/tanh_glibc.rs",
)

#: (area, how its words change); the first match wins.
REPO_AREAS = [
    (r"^docs/parts/", "repo: docs/parts", "history: stays as written"),
    (r"^docs/status-log\.md$", "repo: docs/status-log.md", "history: stays as written"),
    (
        r"^docs/decision-log\.md$",
        "repo: docs/decision-log.md",
        "old entries stay; new ones in the new words",
    ),
    (
        r"^CHANGELOG\.md$",
        "repo: CHANGELOG.md",
        "released sections stay; Unreleased in the new words",
    ),
    (r"^specs/", "repo: specs", "a spec writer (the writer never edits specs/)"),
    (
        r"^(?:" + "|".join(map(re.escape, CLEAN_ROOM_CRATES + CLEAN_ROOM_CORE)) + ")",
        "repo: clean-room code",
        "a clean-room agent, from the writer's list",
    ),
    (
        r"^docs/validation/.*\.json$",
        "repo: docs/validation (records' data)",
        "generated: changes when a record is re-made",
    ),
    (
        r"^docs/benchmarks/.*\.json$",
        "repo: docs/benchmarks (reports' data)",
        "generated: changes when a report is re-made",
    ),
    (
        r"^validation/difftest/",
        "repo: validation/difftest (cases, triage)",
        "identifiers kept; prose notes in the new words",
    ),
    (
        r"^validation/(?:investigations|probes)/",
        "repo: validation/investigations, probes",
        "history: the tools and outputs behind past findings stay as written",
    ),
    (r"^README\.md$", "repo: README.md", "rollout (shared document: listed for the orchestrator)"),
    (r"^docs/api/", "repo: docs/api", "rollout"),
    (
        r"^docs/validation/",
        "repo: docs/validation (records' pages)",
        "rollout; generated parts when re-made",
    ),
    (
        r"^docs/findings/",
        "repo: docs/findings",
        "rollout (restricted files: the writer may read and edit their words)",
    ),
    (r"^docs/", "repo: docs (other current documents)", "rollout"),
    (r"^python/", "repo: python (package: messages, docstrings)", "rollout"),
    (
        r"^validation/src/larmorx_validation/evidence/",
        "repo: evidence module (the site's texts)",
        "rollout, then a re-export (no recomputation)",
    ),
    (r"^validation/", "repo: validation (other)", "rollout"),
    (r"^tests/", "repo: tests", "rollout (names and messages tests check)"),
    (r"^crates-nc/", "repo: crates-nc (FSL replicas)", "rollout"),
    (r"^crates-gpl/", "repo: crates-gpl (tedana replica, tools)", "rollout"),
    (r"^crates/", "repo: crates (not clean-room)", "rollout"),
    (r"^scripts/", "repo: scripts", "rollout"),
    (r"^CLAUDE\.md$", "repo: CLAUDE.md", "the orchestrator, who alone edits it"),
    (r"^PLAN\.md$", "repo: PLAN.md", "rollout"),
    (r"", "repo: other", "rollout"),
]
SITE_AREAS = [
    (r"^content/", "site: content", "rollout"),
    (r"^data/", "site: data", "rollout (status.yml generated from tools.toml)"),
    (r"^templates/", "site: templates", "rollout"),
    (
        r"^static/evidence/",
        "site: static/evidence (exported)",
        "generated by the evidence module: re-export",
    ),
    (r"^static/js/", "site: static/js", "rollout"),
    (r"^site\.yml$", "site: site.yml", "rollout"),
    (r"", "site: other (README, CLAUDE.md, tools)", "rollout"),
]

#: Areas whose readers are users: the jargon is counted there only.
USER_FACING = re.compile(
    r"^(site: (content|data|templates|static|site\.yml)|repo: (README\.md|docs/api|python|"
    r"evidence module|CHANGELOG\.md))"
)


#: Who changes an area's words, from how its words change.
GROUPS = [
    ("the writer, in the rollout", r"^rollout"),
    ("generated: re-made or re-exported", r"^generated"),
    (
        "someone else: a clean-room agent, a spec writer, the orchestrator",
        r"^(a clean-room agent|a spec writer|the orchestrator)",
    ),
    (
        "kept as written: history and identifiers",
        r"^(history|identifiers kept|old entries|released)",
    ),
]


def group_of(how: str) -> str:
    for name, rx in GROUPS:
        if re.match(rx, how):
            return name
    raise AssertionError(how)


VCS = "git"


def tracked(root: Path) -> list[str]:
    out = subprocess.run(
        [VCS, "ls-files", "-z"], cwd=root, capture_output=True, check=True
    ).stdout.decode()
    return sorted(p for p in out.split("\0") if p and not SKIP.search(p))


def area_of(rel: str, areas: list[tuple[str, str, str]]) -> tuple[str, str]:
    for rx, name, how in areas:
        if re.search(rx, rel):
            return name, how
    raise AssertionError(rel)


#: A token with a slash is a path, a URL or a link's target: its words are file names (counted in
#: the table of files named with *original*), not prose.
PATHLIKE = re.compile(r"\S*/\S*")


def prose(rel: str, text: str) -> str:
    """The text whose words a reader sees: for a JSON file its string values that hold a space
    (labels, captions, notes; not keys or identifiers such as a panel's role), paths taken out."""
    if rel.endswith(".json"):
        try:
            data = json.loads(text)
        except ValueError:
            data = None
        if data is not None:
            out: list[str] = []

            def walk(o: object) -> None:
                if isinstance(o, dict):
                    for v in o.values():
                        walk(v)
                elif isinstance(o, list):
                    for v in o:
                        walk(v)
                elif isinstance(o, str) and re.search(r"\s", o):
                    out.append(o)

            walk(data)
            text = "\n".join(out)
    return PATHLIKE.sub(" ", text)


def count(text: str, user_facing: bool) -> Counter:
    c: Counter = Counter()
    spans: list[tuple[int, int]] = []
    for m in CLEAN_ROOM_ORIGINAL.finditer(text):
        c["clean-room original"] += 1
        spans.append(m.span())
    for rx in ORIGINAL_AS_CLEAN_ROOM:
        for m in rx.finditer(text):
            if any(a <= m.start() < b for a, b in spans):
                continue
            c["original = clean room"] += 1
            spans.append(m.span())
    for m in ANY_ORIGINAL.finditer(text):
        if not any(a <= m.start() < b for a, b in spans):
            c["original, to read"] += 1
    c["exact replica"] = len(EXACT_REPLICA.findall(text))
    c["port as a kind"] = len(PORT_AS_KIND.findall(text))
    c["slogan"] = sum(len(rx.findall(text)) for rx in SLOGANS)
    c["tagline"] = len(TAGLINE.findall(text))
    if user_facing:
        for word, rx in JARGON.items():
            n = len(rx.findall(text))
            if n:
                c["jargon"] += n
                c["jargon: " + word] += n
    return +c


COLUMNS = [
    "clean-room original",
    "original = clean room",
    "exact replica",
    "port as a kind",
    "slogan",
    "tagline",
    "jargon",
    "original, to read",
]
CHANGE = COLUMNS[:7]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--site", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()

    rows = []  # (area, how, path, Counter)
    names = []  # tracked paths of this repository with "original" in their name
    texts = {}  # (label, rel) -> (group, raw text)
    heads = {}
    for label, root, areas in (("repo", a.repo, REPO_AREAS), ("site", a.site, SITE_AREAS)):
        heads[label] = subprocess.run(
            [VCS, "rev-parse", "--short=8", "HEAD"],
            cwd=root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        for rel in tracked(root):
            p = root / rel
            if not p.is_file() or p.stat().st_size > TEXT_LIMIT:
                continue
            try:
                text = p.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            area, how = area_of(rel, areas)
            texts[(label, rel)] = (group_of(how), text)
            if label == "repo" and re.search(r"original", Path(rel).name, FLAGS):
                names.append((rel, group_of(how)))
            c = count(prose(rel, text), bool(USER_FACING.match(area)))
            if c:
                rows.append((area, how, f"{label}:{rel}", c))

    # who names each such file, outside what is kept as written
    kept = GROUPS[3][0]
    refs: dict[str, tuple[int, int]] = {}
    for rel, _ in names:
        full, stem = Path(rel).name, Path(rel).stem
        by_full = by_stem = 0
        for (label, other), (g, text) in texts.items():
            if g == kept or (label == "repo" and other == rel):
                continue
            by_full += full in text
            by_stem += stem in text
        refs[rel] = (by_full, by_stem)

    by_area: dict[str, Counter] = defaultdict(Counter)
    how_of: dict[str, str] = {}
    files_of: Counter = Counter()
    for area, how, _path, c in rows:
        by_area[area].update(c)
        how_of[area] = how
        if any(c[k] for k in CHANGE):
            files_of[area] += 1
    total: Counter = Counter()
    for c in by_area.values():
        total.update(c)

    L = []
    L.append("# The old words: an inventory for the documentation revision")
    L.append("")
    L.append(
        "Generated by `validation/investigations/docs-revision/old_words.py` (its README says how "
        f"to run it) from this repository at `{heads['repo']}` and the documentation site at "
        f"`{heads['site']}`. The words and what replaces them: [docs/writing-guide.md]"
        "(../../../docs/writing-guide.md). The counts are of occurrences; a file is listed when "
        "it holds at least one word to change (the first seven columns)."
    )
    L.append("")
    L.append("What each column counts:")
    L.append("")
    L.append("- **clean-room original**: the phrase, singular or plural, however hyphenated.")
    L.append(
        "- **original = clean room**: forms in which *original* can only mean a clean room: the "
        'option\'s value (`implementation="original"`, `--implementation original`, still '
        "accepted for 0.1.0rc1's users, so the code that accepts it stays), a difference's label "
        "(`original − FSL 6.0.7.23`), a status in brackets (`(original)`), *the original's "
        "record* (a record belongs to a port), *LARMORX's original*."
    )
    L.append(
        "- **exact replica**: *exact replica* and *bit-exact replica* (the word *replica* says it)."
    )
    L.append(
        "- **port as a kind**: *ported from the source*, *a port of the source*, *two (three) ways "
        "of reimplementing*: a port is any reimplementation, a replica or a clean room."
    )
    L.append(
        "- **slogan**: *many toolkits*, *one framework*, *common computational framework*, "
        "*clean-room development practices*, *established tools* or *algorithms*, *each keeps its "
        "colour*."
    )
    L.append("- **tagline**: *into Resonance*, the user's tagline (the guide's open question 1).")
    L.append(
        "- **jargon**, counted only where users read (the site's pages, data, templates, scripts "
        "and exported evidence; the README, `docs/api/`, the Python package, the evidence module, "
        "the changelog): *oracle*, *parity*, *golden*, *tier*, *night*, *D13*, *upstream*, "
        "*ecosystem*."
    )
    L.append(
        "- **original, to read**: every other *original* or *originals*. Most name the program "
        "LARMORX is compared with, which is right; some name a clean room in forms the patterns "
        "cannot tell apart (*an original is written from a specification* on the home page, beside "
        "*when an original has a deterministic defect* on the Validation page, which is right). "
        "The rollout reads each one."
    )
    L.append("")
    L.append("## Who changes them")
    L.append("")
    L.append("| Who | Files to change | " + " | ".join(COLUMNS) + " |")
    L.append("|" + "---|" * (2 + len(COLUMNS)))
    for gname, _ in GROUPS:
        gc: Counter = Counter()
        gf = 0
        for area, c in by_area.items():
            if group_of(how_of[area]) == gname:
                gc.update(c)
                gf += files_of[area]
        L.append(f"| {gname} | {gf} | " + " | ".join(str(gc[k]) for k in COLUMNS) + " |")
    L.append("")
    L.append("## Totals by area")
    L.append("")
    head = "| Area | How its words change | Files to change | " + " | ".join(COLUMNS) + " |"
    L.append(head)
    L.append("|" + "---|" * (3 + len(COLUMNS)))
    order = sorted(by_area, key=lambda k: (not k.startswith("site"), k))
    for area in order:
        c = by_area[area]
        L.append(
            f"| {area} | {how_of[area]} | {files_of[area]} | "
            + " | ".join(str(c[k]) for k in COLUMNS)
            + " |"
        )
    L.append(
        f"| **all** | | **{sum(files_of.values())}** | "
        + " | ".join(f"**{total[k]}**" for k in COLUMNS)
        + " |"
    )
    L.append("")
    jw = [(k.split(": ", 1)[1], v) for k, v in total.items() if k.startswith("jargon: ")]
    L.append(
        "The jargon by word (where users read): "
        + ", ".join(f"*{w}* {n}" for w, n in sorted(jw, key=lambda x: -x[1]))
        + "."
    )
    L.append("")

    L.append("## Files named with *original*")
    L.append("")
    L.append(
        "This repository's tracked files whose name holds *original* (the site has none). "
        "*Files naming it*: how many other files of both repositories, outside what is kept as "
        "written, hold its whole name (links) and its name without the extension (links, suite "
        "and module names, identifiers). The writing guide's rule: records, findings, benchmark "
        "reports, tests and validation modules are renamed (*original* becomes *clean-room* or "
        "*clean_room*); random testing's permanent cases (identifiers in the records and the "
        "triage) and the investigations (history) keep their names."
    )
    L.append("")
    L.append("| File | Renamed? | Files naming it (whole name) | (without extension) |")
    L.append("|---|---|---|---|")
    for rel, g in names:
        rename = "kept" if g == kept else "renamed"
        L.append(f"| `{rel}` | {rename} | {refs[rel][0]} | {refs[rel][1]} |")
    L.append("")

    L.append("## File by file")
    L.append("")
    for area in order:
        rs = [r for r in rows if r[0] == area and any(r[3][k] for k in CHANGE)]
        if not rs:
            continue
        L.append(f"### {area}")
        L.append("")
        L.append(f"How its words change: {how_of[area]}.")
        L.append("")
        L.append("| File | " + " | ".join(COLUMNS) + " |")
        L.append("|" + "---|" * (1 + len(COLUMNS)))
        rs.sort(key=lambda r: (-sum(r[3][k] for k in CHANGE), r[2]))
        for _, _, path, c in rs:
            L.append(f"| `{path}` | " + " | ".join(str(c[k]) for k in COLUMNS) + " |")
        L.append("")
    a.out.write_text("\n".join(L), encoding="utf-8")
    print(
        f"{len(rows)} files with words; {sum(files_of.values())} to change; "
        + ", ".join(f"{k} {total[k]}" for k in COLUMNS)
    )


if __name__ == "__main__":
    main()
