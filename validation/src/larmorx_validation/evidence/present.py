# SPDX-License-Identifier: Apache-2.0
"""The writing guide's presentation for evidence pages whose texts follow today's common forms
(``docs/writing-guide.md``, sections 1, 4 and 5; :mod:`words` does the work).

Most pages name their runs in a few shapes: "ANTs 2.6.5, 1 thread, as built" for the original,
"larmorx (port), 12 threads" with "reproduce mode" for LARMORX's port, "larmorx − ANTs 2.6.5,
1 thread, as built" for a comparison and "larmorx against the original" for what it compares.
:func:`example` reads those shapes and gives each panel and comparison the guide's name, its
port and its role label; a page names only what the shapes do not say (its program, its port,
each example's "What this shows" line) and, where a label has another shape, an override.

Presentation only, as in :mod:`words`: texts, the ``port``/``order``/``shows`` fields and the
order of ids change; arrays, metrics and verdicts do not."""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from larmorx_validation.evidence import words


@dataclass
class Page:
    """What a page's shapes do not say."""

    software: str  # "ANTs"
    program: str  # "N4BiasFieldCorrection"
    version: str  # the original's version the record compares with: "2.6.5"
    port: str = "replica"  # LARMORX's port on the page: "replica" or "clean-room"
    package: str = "larmorx, Apache-2.0"  # the port's package and licence, for its panels
    #: a page's own rewordings (pattern, replacement), applied first, before the guide's
    #: words: "(the row above)" named by what that row compares, where the order moves it
    texts: list[tuple[str, str]] = field(default_factory=list)


#: The oracle's words in texts (the guide's section 1): fMRIPrep's runs and the builds of the
#: original, by what they are. Also applied to an example's provenance, which names LARMORX's
#: commits as the package's ("with larmorx `e22378b480f4`") and keeps them.
_ORACLE_TEXTS = [
    (r"\bfMRIPrep (\d[\d.]*)'s oracle run(s?)\b", r"fMRIPrep \1's run\2"),
    (r"\bfMRIPrep (\d[\d.]*) oracle runs\b", r"fMRIPrep \1 runs"),
    (r"\boracle runs\b", "fMRIPrep runs"),
    (r"\boracle run's\b", "fMRIPrep run's"),
    (r"\boracle run\b", "fMRIPrep run"),
    (r"<fMRIPrep fMRIPrep run>", "<fMRIPrep oracle run>"),  # a command's placeholder stays
    (r"\bthe repository's oracle build\b", "the repository's build"),
    (r"\bthe current oracle build\b", "the repository's current build"),
    (r"\boracle build\b", "build"),
    (r"\bthe parity case\b", "the comparison's case"),
    (r"\bparity case\b", "comparison case"),
]

#: The port's name in texts, after :func:`words.reword` (``{port}`` = "replica"). A bare
#: "larmorx" names the port, not a module path (``larmorx.ants``), a code span or the prefix of
#: a message the program prints ("larmorx: N4BiasFieldCorrection: cannot read ...").
_PORT_TEXTS = [
    (r"\blarmorx's reproduce-mode output\b", "the {port}'s output in `reproduce`"),
    (r"\blarmorx's `reproduce`", "the {port}'s `reproduce`"),
    (r"\blarmorx's default, `fix`,", "the {port}'s default, `fix`,"),
    (r"\blarmorx's default\b", "the {port}'s default, `fix`,"),
    (r"\blarmorx's\b", "the {port}'s"),
    (r"\blarmorx \(port\)", "the {port}"),
    (r"(?<![\w./`-])larmorx(?![\w/`-]|\.\w|: \w+:)", "the {port}"),
    (r"^`reproduce` gives\b", "In `reproduce`, the {port} gives"),
    (r"\bReproduce mode gives\b", "In `reproduce`, the {port} gives"),
]


def oracle(s: str | None) -> str | None:
    """A text with the oracle's words replaced (:data:`_ORACLE_TEXTS`)."""
    if not s:
        return s
    for pat, rep in _ORACLE_TEXTS:
        s = re.sub(pat, rep, s)
    return s


def _own(page: Page, s: str | None, code: bool = True) -> str | None:
    """A text with the page's own rewordings (:attr:`Page.texts`)."""
    if not s:
        return s
    for pat, rep in page.texts:
        s = re.sub(pat, rep if code else rep.replace("`", ""), s, flags=re.M)
    return s


def text(page: Page, s: str | None, code: bool = True) -> str | None:
    """A text of the page in the guide's words, LARMORX's port named: the page's own
    rewordings, then the guide's (:func:`words.reword`), the oracle's words and the port's."""
    if not s:
        return s
    s = oracle(words.reword(_own(page, s, code), code=code)) or ""
    name = words.PORT_NAME[page.port]
    for pat, rep in _PORT_TEXTS:
        if not code:
            rep = rep.replace("`", "")
        s = re.sub(pat, rep.format(port=name), s, flags=re.M)
    # a sentence that starts with the port's name starts with a capital
    return re.sub(rf"(^|[.!?] |\n)the {re.escape(name)}\b", rf"\1The {name}", s)


# ------------------------------------------------------------------------------- the panels

_MODES = [
    (r"\bdefault mode\b", "fix (the default)"),
    (r"^default\b", "fix (the default)"),
    (r"\breproduce mode\b", "reproduce"),
]


def _mode(sub: str) -> str:
    for pat, rep in _MODES:
        sub = re.sub(pat, rep, sub)
    return sub


def _original_panel(page: Page, p: dict) -> tuple[str, str, float]:
    """An original's panel: its name (the guide's "ANTs N4BiasFieldCorrection 2.6.5"), its
    second line (how it ran) and its place among the original's panels (a version first, then
    fMRIPrep's build, then correctly rounded maths)."""
    label, sub = p["label"], p.get("sub") or ""
    program = page.program
    m = re.match(r"^(\w+): (.*)$", label)  # "Atropos: ANTs 2.6.5, 1 thread"
    if m and m.group(1) != page.software:
        program, label = m.group(1), m.group(2)
    m = re.match(
        rf"^{re.escape(page.software)}(?: (?P<ver>[0-9][\w.+-]*))?(?P<img> \(fMRIPrep's image\))?"
        r"(?P<rest>,.*)?$",
        label,
    )
    if not m:
        raise ValueError(f"an original's panel of another shape: {p['label']!r}")
    ver = m.group("ver") or page.version
    rest = [r.strip() for r in (m.group("rest") or "").split(",") if r.strip()]
    tail, rank = [], 0.0
    if m.group("img"):
        tail.append("fMRIPrep's build")
        rank = 1.0
    if "correctly rounded maths" in rest:
        rest.remove("correctly rounded maths")
        tail.append("correctly rounded maths")
        rank = max(rank, 2.0)
    elif p["role"] == "reference":
        rank = 2.0
    name = f"{page.software} {program} {ver}" + "".join(f", {t}" for t in tail)
    sub = text(page, sub, code=False) or ""
    second = " · ".join(x for x in (", ".join(rest), sub) if x)
    return name, second, rank


def _port_panel(page: Page, p: dict, against: str | None) -> tuple[str, str]:
    label, sub = p["label"], p.get("sub") or ""
    m = re.match(r"^(?:(\w+): )?larmorx(?: \(port\))?(?P<rest>,.*)?$", label)
    if not m:
        raise ValueError(f"a port's panel of another shape: {label!r}")
    rest = [r.strip() for r in (m.group("rest") or "").split(",") if r.strip()]
    mode = _mode(sub)
    if "fix" not in mode and "reproduce" not in mode:
        raise ValueError(f"a port's panel without its mode: {label!r} ({sub!r})")
    how = ", ".join([*rest, mode])
    second = f"{page.package} · {how}" + (f" · against {against}" if against else "")
    name = words.PORT_NAME[page.port]
    if m.group(1):  # "Atropos: larmorx (port)"
        name = f"{m.group(1)}: {name}"
    return name, second


def _fmriprep_panel(p: dict) -> tuple[str, str]:
    run = p["label"].removesuffix("'s own output")
    return "fMRIPrep's output", f"{run}: {p['sub']}" if p.get("sub") else run


# -------------------------------------------------------------------------- the comparisons

_LABEL = [
    (r"\blarmorx's default\b", "{port}, fix"),
    (r"\blarmorx default mode\b", "{port}, fix"),
    (r"\blarmorx default\b", "{port}, fix"),
    (r"\blarmorx reproduce\b", "{port}, reproduce"),
    (r"\blarmorx \(reproduce\)", "{port}, reproduce"),
    (r"\blarmorx \((--[\w-]+ [\w.]+)\)", r"{port} (\1)"),
    (r"\blarmorx (--[\w-]+ [\w.]+|double),", r"{port}, \1,"),  # "larmorx --float 1, 12 threads"
    (r"\blarmorx\b", "{port}"),
    (r"^(\w+ [0-9][\w.]*) (--[\w-]+ [\w.]+|double),", r"\1, \2,"),  # "ANTs 2.6.5 double, ..."
    (r"\bdefault − reproduce mode\b", "fix − reproduce"),
    (r"\bdefault mode − reproduce mode\b", "fix − reproduce"),
    (r"\b, default − reproduce\b", ", fix − reproduce"),
    (r"\breproduce mode\b", "reproduce"),
    (r"\bfMRIPrep's own output\b", "fMRIPrep's output"),
    (r"\bfMRIPrep's own \(", "fMRIPrep's output ("),
]


def _label(page: Page, old: str) -> str:
    """A comparison's label: LARMORX's port by its name, the original by its program."""
    s = old
    for pat, rep in _LABEL:
        s = re.sub(pat, rep.format(port=words.PORT_NAME[page.port]), s)
    # the first mention of the software names the program; a second one is the same program
    sw = re.escape(page.software)
    first = re.search(rf"\b{sw}\b", s)
    if first:
        head, tail = s[: first.end()], s[first.end() :]
        tail = re.sub(rf"\b{sw} (?=[0-9])", "", tail)
        s = f"{head} {page.program}{tail}" if not tail.startswith(f" {page.program}") else s
    return s


_THREADS = re.compile(r"(\d+) threads?(?: [^−]*)? − (\d+) threads?")
_IDS = re.compile(r"\(((?:[A-Z]+-\d+)(?:(?:, | and )[A-Z]+-\d+)*)\)")


def _what(old: str, a: dict | None, b: dict | None) -> str | None:
    """What changes between two runs of one program, from a comparison's old label."""
    if m := _THREADS.search(old):
        n = sorted({int(m.group(1)), int(m.group(2))})
        return f"{n[0]} and {n[1]} threads" if len(n) == 2 else None
    if re.search(r"\brun \d+ − run \d+\b|another run − run \d+", old):
        return "two runs of the same command"
    if a is not None and a.get("role") == "fmriprep":
        return "fMRIPrep's run and the repository's build"
    if "fMRIPrep's image" in old or re.search(r"\b2\.6\.2\b.*− .*\b2\.6\.5\b", old):
        return "fMRIPrep's build and the repository's"
    if "correctly rounded" in old or "CORE-MATH" in old:
        return "correctly rounded maths and glibc's"
    if "--float 1 − double" in old:
        return "--float 1 and double"
    if "clock's seed − " in old:
        return "the clock's seed and a fixed seed"
    if "the clock's seed: run" in old:
        return "two runs with the clock's seed"
    return None


def _role(page: Page, d: dict, pa: str, pb: str, panels: dict) -> tuple[str, str]:
    """A comparison's role label (the guide's section 4) and what it is compared with, for the
    metrics table's groups."""
    old, who = d["label"], words.PORT_NAME.get(pa)
    a, b = panels.get(d["a"]), panels.get(d["b"])
    if who and pb == pa:
        if "thread" in old and not re.search(r"default|reproduce|fix", old.split("−")[0]):
            what = _what(old, a, b)
        elif re.search(r"\b(default|fix)\b.*− .*reproduce|reproduce.*− .*default", old):
            ids = _IDS.search(old)
            what = "fix and reproduce" + (f" ({ids.group(1)})" if ids else "")
        else:
            what = _what(old, a, b)
        if not what:
            raise ValueError(f"{d['id']}: what changes in {old!r}?")
        return f"the {who} against itself: {what}", "itself"
    if who and pb in ("original", "fmriprep") and pb == "fmriprep":
        return f"the {who} against fMRIPrep's output", "fMRIPrep's output"
    if who and pb == "original":
        role = f"the {who} against the original"
        if b is not None and b.get("role") == "reference":
            role += ": correctly rounded maths"
        elif b is not None and "fMRIPrep's image" in b.get("label", ""):
            role += ": fMRIPrep's build"
        return role, "the original"
    if who and pb == "input":
        return f"the {who} against the known answer", "the known answer"
    if pa in ("original", "fmriprep") and pb == "input":
        return "the original against the known answer", "the known answer"
    if pa in ("original", "fmriprep") and pb in ("original", "fmriprep"):
        what = _what(old, a, b)
        if not what:
            raise ValueError(f"{d['id']}: what changes in {old!r}?")
        return f"the original against itself: {what}", "itself"
    raise ValueError(f"{d['id']}: a comparison of {pa} and {pb}")


def example(
    page: Page,
    ex: dict,
    *,
    shows: str,
    verdicts: list[str],
    title: str | None = None,
    runs: dict[str, str] | None = None,
    panels: dict[str, tuple[str, str | None]] | None = None,
    diffs: dict[str, tuple[str, str]] | None = None,
) -> dict:
    """One worked example of ``page`` in the guide's words and order. ``shows`` and
    ``verdicts`` as :func:`words.example`; ``title`` the example's new title; ``runs`` the
    port of a run a comparison names without a panel (by default: a variability comparison's
    is the original's, a check's the port of the run it is compared with); ``panels`` and
    ``diffs`` override what the shapes give."""
    # the page's own rewordings first: :func:`words.example` rewords the texts in the guide's
    # words, whose forms may otherwise take a page's phrase for another page's
    for d in ex["diffs"]:
        d["reason"] = _own(page, d.get("reason"))
    ex["summary"] = _own(page, ex.get("summary"))
    for f in ex.get("facts", []):
        f["text"] = _own(page, f["text"])
    ex["metrics_note"] = _own(page, ex.get("metrics_note"))
    if ex.get("chain"):
        ex["chain"]["note"] = _own(page, ex["chain"].get("note"))
    by_id = {p["id"]: p for p in ex["panels"]}
    ports, new, rank = {}, {}, {}
    main_b = {}
    for d in ex["diffs"]:
        if d["kind"] == "main" and d["a"] in by_id and d["a"] not in main_b:
            main_b[d["a"]] = d["b"]
    originals = {}
    for pid, p in by_id.items():
        if p["role"] in ("original", "reference"):
            name, sub, r = _original_panel(page, p)
            new[pid], originals[pid], rank[pid] = (name, sub), name, r
        elif p["role"] == "input":
            new[pid] = (p["label"], text(page, p.get("sub"), code=False))
        elif p["role"] == "fmriprep":
            new[pid] = _fmriprep_panel(p)
            originals[pid] = "fMRIPrep's output"
    for pid, p in by_id.items():
        if p["role"] == "larmorx":
            ports[pid] = page.port
            against = originals.get(main_b.get(pid, ""))
            new[pid] = _port_panel(page, p, against)
    new.update(panels or {})

    def port_of(pid: str) -> str:
        if pid in ports:
            return ports[pid]
        if pid in by_id:
            return words.ROLE_PORT[by_id[pid]["role"]]
        return hidden[pid]

    hidden: dict[str, str] = dict(runs or {})
    for d in ex["diffs"]:
        for end, other in (("a", "b"), ("b", "a")):
            pid = d[end]
            if pid in by_id or pid in hidden:
                continue
            if d["kind"] == "variability":
                hidden[pid] = "original"
            elif d[other] in by_id:
                hidden[pid] = port_of(d[other])
            else:
                raise ValueError(f"{ex['id']}: run {pid} of {d['id']} has no port")
    out, target = {}, {}
    for d in ex["diffs"]:
        pa, pb = port_of(d["a"]), port_of(d["b"])
        role, target[d["id"]] = _role(page, d, pa, pb, by_id)
        out[d["id"]] = (_label(page, d["label"]), role)
    for k, v in (diffs or {}).items():
        out[k] = v
        target[k] = v[1].split(" against ", 1)[1].split(":")[0]
    # the metrics table's groups: the original against itself, then the port's comparisons
    order = {p: (words.PORT_ORDER.index(port_of(p)), rank.get(p, 0.0), i)
             for i, p in enumerate(by_id)}  # fmt: skip
    targets = ["the original", "fMRIPrep's output", "the known answer", "itself", "the replica"]

    def place(pid: str) -> tuple:
        return order.get(pid, (words.PORT_ORDER.index(port_of(pid)), 9.0, 99))

    groups = []
    for port in ("original", page.port):
        ds = [
            d
            for d in ex["diffs"]
            if (port_of(d["a"]) in ("original", "fmriprep")) == (port == "original")
        ]
        if not ds:
            continue
        ds.sort(key=lambda d: (targets.index(target[d["id"]]), place(d["a"]), place(d["b"])))
        if port == "original":
            title_ = "The original against itself: the yardstick"
            if all(target[d["id"]] == "the known answer" for d in ds):
                title_ = "The original against the known answer"
        else:
            # the port against the others, then against itself (its threads, fix and reproduce)
            who = words.PORT_NAME[port]
            others = [d for d in ds if target[d["id"]] != "itself"]
            itself = [d for d in ds if target[d["id"]] == "itself"]
            if others:
                seen = list(dict.fromkeys(target[d["id"]] for d in others))
                title_ = f"The {who} against " + (
                    seen[0] if len(seen) == 1 else ", ".join(seen[:-1]) + " and " + seen[-1]
                )
                groups.append((title_, [d["id"] for d in others]))
            if itself:
                whats = [out[d["id"]][1].split("against itself: ", 1)[-1] for d in itself]
                kinds = {
                    "thread counts" if w.endswith(" threads") else w.split(" (")[0] for w in whats
                }
                order = ["thread counts", "fix and reproduce", *sorted(kinds)]
                named = ", ".join(k for k in dict.fromkeys(order) if k in kinds)
                groups.append((f"The {who} against itself: {named}", [d["id"] for d in itself]))
            continue
        groups.append((title_, [d["id"] for d in ds]))
    words.example(
        ex,
        ports=ports,
        panels=new,
        diffs=out,
        groups=groups,
        shows=words.link_ids(shows),
        verdicts=verdicts,
        rank=rank,
        runs={k: v for k, v in hidden.items() if k not in by_id},
    )
    if title:
        ex["title"] = title
    # the port's own words in the example's texts (words.example has reworded them)
    ex["summary"] = text(page, ex.get("summary"))
    for f in ex.get("facts", []):
        f["label"] = text(page, f["label"], code=False)
        f["text"] = text(page, f["text"])
    ex["metrics_note"] = text(page, ex.get("metrics_note"))
    for d in ex["diffs"]:
        d["reason"] = text(page, d.get("reason"))
    ex["provenance"] = [oracle(x) for x in ex.get("provenance") or []]
    if ex.get("poster", {}).get("caption"):
        ex["poster"]["caption"] = text(page, ex["poster"]["caption"], code=False)
    if ex.get("chain"):
        chain(page, ex["chain"])
    return ex


def chain(page: Page, ch: dict) -> None:
    """A worked example's chain of causes (what separates fMRIPrep's output from the port's):
    each run named as the panels name it, LARMORX's by its port (``port`` on every node). The
    site shows its title and nodes as plain text."""
    who = words.PORT_NAME[page.port]
    ch["title"] = text(page, ch["title"], code=False)
    for step in ch["steps"]:
        for n in step["nodes"]:
            if n["role"] == "larmorx":
                m = re.match(r"^larmorx(?: \(port\))?(?P<rest>,.*)?$", n["label"])
                if not m:
                    raise ValueError(f"a chain's run of another shape: {n['label']!r}")
                n["label"] = who + _mode(m.group("rest") or "")
                n["port"] = page.port
            elif n["role"] in ("original", "reference"):
                name, second, _ = _original_panel(page, {"label": n["label"], "role": n["role"]})
                n["label"] = f"{name}, {second}" if second else name
                n["port"] = "original"
            elif n["role"] == "fmriprep":
                n["port"] = "fmriprep"
            else:
                raise ValueError(f"a chain's run of the role {n['role']!r}")
            n["sub"] = text(page, n.get("sub"), code=False)
    for link in ch["links"]:
        link["cause"] = text(page, link["cause"], code=False)
    ch["note"] = text(page, ch.get("note"))


#: A command's placeholder for the directory of fMRIPrep's runs, by what it is.
_RUN_PLACEHOLDER = ("<fMRIPrep oracle run>", "<fMRIPrep run>")


def cases(page: Page, cs: list[dict]) -> None:
    """Every case's texts in the guide's words, its command's placeholders included."""
    for c in cs:
        if c.get("command"):
            c["command"] = c["command"].replace(*_RUN_PLACEHOLDER)
        c["description"] = text(page, c.get("description"), code=False)
        for r in c["results"].values():
            r["reason"] = text(page, r.get("reason"))


def table(page: Page, t: dict) -> None:
    t["title"] = text(page, t.get("title"))
    t["note"] = text(page, t.get("note"))
    for r in t["rows"]:
        r["head"] = text(page, r.get("head"))
        if r.get("sub"):
            r["sub"] = text(page, r["sub"])
        for c in r["cells"]:
            if "text" in c:
                c["text"] = text(page, c["text"])


def original_name(page: Page, code: bool = True) -> str:
    """The original in a title: "ANTs `N4BiasFieldCorrection` 2.6.5"."""
    prog = f"`{page.program}`" if code else page.program
    return f"{page.software} {prog} {page.version}"


def implementation(page: Page, im: dict, comparison: str) -> None:
    """An agreement chart's title and names (the guide's section 4)."""
    who = words.PORT_NAME[page.port]
    im["title"] = f"Agreement, {who} against the original: {original_name(page)}"
    im["comparison"] = comparison
    im["label"] = f"{who} against the original: {original_name(page, code=False)} ({comparison})"
    im["short"] = who
    im["port"] = page.port


def speed(page: Page, sp: dict | None, caption: str, threads: str = "") -> None:
    """A speed chart's title, caption and marks: the port's points by its name."""
    if not sp:
        return
    who = words.PORT_NAME[page.port]
    sp["title"] = f"Speed, the {who} against the original: {original_name(page)}{threads}"
    sp["caption"] = caption
    marks = {"larmorx": page.port, "larmorx-open": f"{page.port}-open"}
    for ser in sp["series"]:
        ser["mark"] = marks.get(ser["mark"], ser["mark"])
        ser["port"] = ser["mark"].removesuffix("-open")
        lab = re.sub(r"^larmorx\b", who, ser["label"])
        ser["label"] = re.sub(
            rf"^{re.escape(page.software)}(?=,)", f"{page.software} {page.program}", lab
        )
    sp["note"] = text(page, sp.get("note"))


#: A category of a record's cases as the pages name it (the id stays the case explorer's
#: filter); a page adds its own.
CATEGORY_LABELS = {
    "fmriprep": "fMRIPrep's calls",
    "options": "options",
    "inputs": "inputs",
    "errors": "errors",
    "formats": "file formats",
    "defects": "defects of the original",
    "real": "real images",
    "difftest": "random testing",
    "panel-fmriprep": "data panel: fMRIPrep's calls",
    "panel-images": "data panel: other images",
}


def random_testing(page: Page, doc: dict, names: dict[str, str]) -> None:
    """The random-testing panel's title by the guide's pattern, and each difference's port by
    its name: ``names`` maps the run's implementation ids (``port``, ``original``) to them."""
    who = [words.PORT_NAME[p] for p in dict.fromkeys(names.values())]
    a = f"the {who[0]}" if len(who) == 1 else "both ports"
    doc["random_testing"] = {
        "title": f"Random testing, {a} against the original: {original_name(page)}",
        "names": {k: words.PORT_NAME[v] for k, v in names.items()},
    }


def finish(page: Page, doc: dict, labels: dict[str, str] | None = None) -> dict:
    """The page's tables, cases, its categories' names and the mark that it is presented."""
    for t in doc.get("tables", []):
        table(page, t)
    cases(page, doc.get("cases", []))
    known = {**CATEGORY_LABELS, **(labels or {})}
    used = list(dict.fromkeys(c["category"] for c in doc.get("cases", [])))
    missing = [c for c in used if c not in known]
    if missing:
        raise ValueError(f"categories without a name: {missing}")
    doc["category_labels"] = {c: known[c] for c in used}
    doc["presentation"] = words.PRESENTATION
    return doc


def _owner(name: str) -> str:
    """ANTs' and AFNI's: the possessive of a software's name."""
    return f"{name}'" if name.endswith("s") else f"{name}'s"


def _sig(v: float, digits: int = 2) -> str:
    return f"{v:.{digits}g}" if v else "0"


def panel_shows(page: Page, ex: dict) -> tuple[str, list[str]]:
    """The "What this shows" line of a data-panel example (``ants_panel``'s shapes: the port
    against the original as built ``lx-ants`` and with correctly rounded maths ``lx-cr``, its
    threads ``lx1-lx``, ``fix`` against ``reproduce`` ``fix-lx``), from its own verdicts,
    metrics and reasons; and the verdict that stands beside it."""
    d = {x["id"]: x for x in ex["diffs"]}
    who = words.PORT_NAME[page.port]
    sw = _owner(page.software)
    main = d["lx-ants"]
    head = "In `reproduce`"
    if "lx1-lx" in d and d["lx1-lx"]["verdict"] == "bit-identical":
        n = sorted({int(x) for x in re.findall(r"(\d+) threads?", d["lx1-lx"]["label"])})
        if len(n) == 2:
            head += f", on {n[0]} and {n[1]} threads"
    parts = []
    if main["verdict"] == "bit-identical":
        parts.append(
            f"{head}, the {who} gives {sw} bytes, as built and with correctly rounded maths."
        )
    elif main["verdict"] == "identical-cr":
        big = max(m["max_abs"] for m in main["metrics"].values())
        parts.append(
            f"{head}, the {who} gives {sw} bytes with correctly rounded maths; {page.software} as "
            f"built differs only where glibc's maths is not correctly rounded (largest difference "
            f"{_sig(big)})."
        )
    else:
        raise ValueError(f"{ex['id']}: lx-ants is {main['verdict']}")
    fix = d.get("fix-lx")
    if fix and fix["verdict"] == "bit-identical":
        parts.append("No correction by default changes this call.")
    elif fix and fix["verdict"] == "d13":
        ids = re.findall(r"\b(?:ANTS|ITK)-\d+\b", fix.get("reason") or "")
        named = " and ".join(dict.fromkeys(ids))
        parts.append(f"Its default, `fix`, corrects {named} here.")
    elif fix:
        raise ValueError(f"{ex['id']}: fix-lx is {fix['verdict']}")
    return " ".join(parts), ["lx-ants"]
