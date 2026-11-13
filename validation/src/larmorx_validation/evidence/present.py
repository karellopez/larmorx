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
    #: a page of several programs (ImageMath and the filters): their names, for the titles
    #: of what covers them all; ``program`` is then empty on the page and named per example
    programs: tuple[str, ...] = ()
    #: a page whose original is a part of its software, not a program (ITK's "transform
    #: files"): the titles name it after the version, "ITK 5.4.5's transform files"
    part: str = ""
    #: each port's package and licence where it is not ``package`` (a page of two ports: the
    #: replica's "larmorx-nc, FSL Licence", the clean room's "larmorx, Apache-2.0")
    packages: dict[str, str] = field(default_factory=dict)
    #: every comparison's label names the version of the original's run ("replica (fix) − AFNI
    #: 3dvolreg 26.3.05, LPS"), not only in an example of two versions
    versions_in_labels: bool = False

    def package_of(self, port: str) -> str:
        """The package and licence of ``port``'s panels."""
        if port in self.packages:
            return self.packages[port]
        return self.package if port == self.port else "larmorx, Apache-2.0"


#: The oracle's words in texts (the guide's section 1): fMRIPrep's runs and the builds of the
#: original, by what they are. Also applied to an example's provenance, which names LARMORX's
#: commits as the package's ("with larmorx `e22378b480f4`") and keeps them.
_ORACLE_TEXTS = [
    (r"\bfMRIPrep (\d[\d.]*)'s oracle run(s?)\b", r"fMRIPrep \1's run\2"),
    (r"\bfMRIPrep (\d[\d.]*) oracle runs\b", r"fMRIPrep \1 runs"),
    (r"\boracle runs\b", "fMRIPrep runs"),
    (r"\boracle run's\b", "fMRIPrep run's"),
    (r"\boracle run\b", "fMRIPrep run"),
    (r"<fMRIPrep fMRIPrep run>", "<fMRIPrep run>"),  # a command's placeholder, as in the cases
    (r"\bthe repository's oracle build\b", "the repository's build"),
    (r"\bthe current oracle build\b", "the repository's current build"),
    (r"\boracle build\b", "build"),
    (r"\bthe parity case\b", "the comparison's case"),
    (r"\bparity case\b", "comparison case"),
]

#: The port's name in texts, after :func:`words.reword` (``{port}`` = "replica"). A bare
#: "larmorx" names the port, not a module path (``larmorx.ants``), a code span or the prefix of
#: a message the program prints ("larmorx: N4BiasFieldCorrection: cannot read ...", the notice
#: "larmorx: upstream defect ANTS-62 (antsRegistration) reproduced: ...") or the package at a
#: commit ("larmorx `6d4f2edb8992`", as a random-testing run's note names its build).
_PORT_TEXTS = [
    (r"\blarmorx's reproduce-mode output\b", "the {port}'s output in `reproduce`"),
    (r"\blarmorx's `reproduce`", "the {port}'s `reproduce`"),
    (r"\blarmorx's default, `fix`,", "the {port}'s default, `fix`,"),
    (r"\blarmorx's default\b", "the {port}'s default, `fix`,"),
    (r"\blarmorx's\b", "the {port}'s"),
    (r"\blarmorx \(port\)", "the {port}"),
    (
        r"(?<![\w./`-])larmorx(?![\w/`-]|\.\w|: \w+:|: upstream defect | `[0-9a-f]{7,}`)",
        "the {port}",
    ),
    (r"^`reproduce` gives\b", "In `reproduce`, the {port} gives"),
    (r"\bReproduce mode gives\b", "In `reproduce`, the {port} gives"),
]


#: Corrections that change no value on the calls the pages show, and only warn there: what the
#: default does, in the register's words. ANTS-36 (the register's row and section): `-r` without
#: a mask does nothing in both modes, and a warning says so; its other half, an unreadable mask,
#: is an error, which no worked example meets.
WARNS = {"ANTS-36": "warns that `-r` does nothing without a mask"}

_CORRECTS = re.compile(r"\bcorrects ((?:[A-Z]+-\d+)(?:, [A-Z]+-\d+)*)( \(\[register\]\([^)]*\)\))?")


def warnings(s: str | None) -> str | None:
    """A reason's "corrects ANTS-36, ANTS-40" said as it is: the corrections that change
    values, and what the default does for those that only warn (:data:`WARNS`)."""
    if not s:
        return s

    def rep(m: re.Match[str]) -> str:
        ids = m.group(1).split(", ")
        fixed = [i for i in ids if i not in WARNS]
        warned = [i for i in ids if i in WARNS]
        if not warned:
            return m.group(0)
        said = " and ".join(f"{WARNS[i]} ({i})" for i in warned)
        if not fixed:
            return f"{said}{m.group(2) or ''}"
        return f"corrects {' and '.join(fixed)}{m.group(2) or ''} and {said}"

    return _CORRECTS.sub(rep, s)


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


def text(page: Page, s: str | None, code: bool = True, port: str = "") -> str | None:
    """A text of the page in the guide's words, LARMORX's port named: the page's own
    rewordings, then the guide's (:func:`words.reword`), the oracle's words and the port's
    (``port``: the port a comparison's reason is about, by default the page's; ``"both"``: a
    text about both ports of the page, where a bare "larmorx" is the project, LARMORX)."""
    if not s:
        return s
    s = oracle(words.reword(_own(page, s, code), code=code)) or ""
    if port == "both":
        return re.sub(
            r"(?<![\w./`-])larmorx(?![\w/`-]|\.\w|: \w+:|: upstream defect | `[0-9a-f]{7,}`)",
            "LARMORX",
            s,
        )
    name = words.PORT_NAME[port or page.port]
    for pat, rep in _PORT_TEXTS:
        if not code:
            rep = rep.replace("`", "")
        s = re.sub(pat, rep.format(port=name), s, flags=re.M)
    # a sentence that starts with the port's name starts with a capital
    return re.sub(rf"(^|[.!?] |\n)the {re.escape(name)}\b", rf"\1The {name}", s)


# ------------------------------------------------------------------------------- the panels

_MODES = [
    # "default mode (ANTS-112 corrected)": one bracket
    (r"\bdefault mode \(([^()]*\bcorrected)\)", r"fix (the default, \1)"),
    # "D13" is the decision's number: `fix` is its corrections
    (r"\bdefault mode \(D13 corrections\)", "fix (the default)"),
    (r"\bdefault mode: corrected\b", "fix (the default), corrected"),
    (r"\bdefault mode\b", "fix (the default)"),
    (r"^default\b", "fix (the default)"),
    (r"\breproduce mode\b", "reproduce"),
]


def _mode(sub: str) -> str:
    for pat, rep in _MODES:
        sub = re.sub(pat, rep, sub)
    return sub


def _original_parts(page: Page, p: dict) -> tuple[str, str, list[str], list[str], float]:
    """An original's panel read from its label: its program, its version, how it ran, its
    builds' words and its place among the original's panels (a version first, then fMRIPrep's
    build, then correctly rounded maths). The label's shapes: "ANTs 2.6.5, 1 thread",
    "Atropos: ANTs 2.6.5", "AFNI 25.2.09, 3dTshift" (the program after the version), "mcflirt,
    FSL 6.0.7.17" (before the software), "AFNI 25.2.09 in fMRIPrep's image" (or "(fMRIPrep's
    image)"), "AFNI in fMRIPrep's image" (its version in the second line: "AFNI_25.2.09"),
    "ANTs 2.6.5's blobs" (what the panel shows of the run)."""
    label, sub = p["label"], p.get("sub") or ""
    sw = re.escape(page.software)
    program = page.program
    m = re.match(r"^(\w+): (.*)$", label)  # "Atropos: ANTs 2.6.5, 1 thread"
    if m and m.group(1) != page.software:
        program, label = m.group(1), m.group(2)
    m = re.match(rf"^(\w+), ({sw}\b.*)$", label)  # "mcflirt, FSL 6.0.7.17"
    if m:
        program, label = m.group(1), m.group(2)
    m = re.match(
        rf"^{sw}(?: (?P<ver>[0-9][\w.+-]*))?(?P<img> \(fMRIPrep's image\)| in fMRIPrep's image)?"
        r"(?:'s (?P<own>[\w -]+))?(?P<rest>,.*)?$",
        label,
    )
    if not m:
        raise ValueError(f"an original's panel of another shape: {p['label']!r}")
    ver = m.group("ver")
    if not ver:  # "AFNI in fMRIPrep's image": "fMRIPrep 25.2.6's build (AFNI_25.2.09 ...)"
        v = re.search(rf"\b{sw}_(\d[\w.]*\d)", sub)
        ver = v.group(1) if v else page.version
    rest = [r.strip() for r in (m.group("rest") or "").split(",") if r.strip()]
    if rest and (rest[0] in page.programs or rest[0].split(" ")[0] in page.programs):
        program = rest[0].split(" ")[0]  # "FSL 6.0.7.23, applytopup" on topup's page
    if rest and program and rest[0].split(" ")[0] == program:
        # "AFNI 25.2.09, 3dTshift": the program is in the name; "topup --nthr=4": its arguments
        args = rest.pop(0).removeprefix(program).strip()
        if args:
            rest.insert(0, args)
    if m.group("own"):  # "ANTs 2.6.5's blobs": what the panel shows of the original's run
        rest.insert(0, f"its {m.group('own')}")
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
    return program, ver, rest, tail, rank


#: What an original's run is to the ports, in its second line: the version they follow.
_FOLLOWED = [
    (r"\bboth implementations' reference\b", "the version both ports follow"),
    (r"\bthe replica's reference\b", "the version the replica follows"),
]


def _original_panel(page: Page, p: dict) -> tuple[str, str, float]:
    """An original's panel: its name (the guide's "ANTs N4BiasFieldCorrection 2.6.5"), its
    second line (how it ran) and its place among the original's panels. A panel of the
    original's kind that names no run of it ("Initial alignment (-init)", the starting point of
    a registration) keeps its label as its name, after the original's runs."""
    label = p["label"]
    if (
        not label.startswith(page.software)
        and re.match(r"^[A-Z][a-z]", label)
        and not re.search(r"\d+\.\d+", label)
    ):
        return label, text(page, p.get("sub"), code=False) or "", 3.0
    program, ver, rest, tail, rank = _original_parts(page, p)
    # a page of several programs names a call's program in the example, not on each panel
    name = " ".join(x for x in (page.software, program, ver) if x) + "".join(f", {t}" for t in tail)
    sub = p.get("sub") or ""
    for pat, rep in _FOLLOWED:
        sub = re.sub(pat, rep, sub)
    sub = text(page, sub, code=False) or ""
    second = " · ".join(x for x in (", ".join(rest), sub) if x)
    return name, second, rank


#: A LARMORX panel's label: an optional program, LARMORX, its package, its port ("larmorx
#: replica", "larmorx-nc replica", "LARMORX clean-room original", "larmorx's clean-room
#: original", "larmorx (replica)", "larmorx (port)", "larmorx replica (larmorx-gpl)"), what the
#: panel shows of the run ("'s series") and how it ran (", -peels 0", ", 1 thread").
_PORT_LABEL = re.compile(
    r"^(?:(?P<prog>\w+): )?(?:LARMORX|larmorx)(?P<pkg>-nc|-gpl)?"
    r"(?P<kind>(?:'s)? clean-room original| clean room| replica| \((?:port|replica|clean room)\))?"
    r"(?: \((?P<pkg2>larmorx-\w+)\))?(?:'s (?P<own>[\w -]+))?(?P<rest>,.*)?$"
)


def port_of_label(page: Page, label: str) -> str:
    """The port a LARMORX panel's label names, or the page's own (``Page.port``) where it names
    none ("larmorx (port), 1 thread")."""
    m = _PORT_LABEL.match(label)
    if not m:
        raise ValueError(f"a port's panel of another shape: {label!r}")
    kind = m.group("kind") or ""
    if "clean" in kind:
        return "clean-room"
    if "replica" in kind:
        return "replica"
    return page.port


def _port_panel(
    page: Page, p: dict, against: str | None, port: str | None = None
) -> tuple[str, str]:
    """A port's panel: its name ("replica", "clean room") and its second line (its package and
    licence, how it ran, what it is compared with)."""
    label, sub = p["label"], p.get("sub") or ""
    m = _PORT_LABEL.match(label)
    if not m:
        raise ValueError(f"a port's panel of another shape: {label!r}")
    port = port or port_of_label(page, label)
    rest = [r.strip() for r in (m.group("rest") or "").split(",") if r.strip()]
    if m.group("own"):  # "larmorx's blobs": what the panel shows of the port's run
        rest.insert(0, f"its {m.group('own')}")
    mode = _mode(sub)
    if "fix" not in mode and "reproduce" not in mode:
        raise ValueError(f"a port's panel without its mode: {label!r} ({sub!r})")
    how = ", ".join([*rest, mode])
    second = f"{page.package_of(port)} · {how}" + (f" · against {against}" if against else "")
    name = words.PORT_NAME[port]
    prog = m.group("prog")
    if prog and prog != page.program:  # "antsAffineInitializer: larmorx" on antsAI's
        name = f"{prog}: {name}"
    return name, second


def _fmriprep_panel(page: Page, p: dict) -> tuple[str, str]:
    run = p["label"].removesuffix("'s own output")
    m = re.match(r"^(\w+): (.*)$", run)  # "antsAI: fMRIPrep 25.2.6, as it ran"
    if m and m.group(1) in (page.program, *page.programs):
        run = m.group(2)
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
    # the modes of two ports' comparisons ("clean-room original, default − replica, default")
    (r"\bfMRIPrep's own (?:reference|file|files)\b", "fMRIPrep's output"),
    (r"\bfMRIPrep's own$", "fMRIPrep's output"),
    (r"\(both default mode\)", "(both fix)"),
    (r"\bboth in their default mode\b", "both fix"),
    (r"\bboth default\b", "both fix"),
    (r"\(default\)", "(fix)"),
    (r"([,:]) default − ", r"\1 fix − "),
    (r", default$", ", fix"),
    (r"\bdefault mode\b", "fix"),
]

#: A port's words at either end of a comparison's label, by the port the comparison's run has
#: ("original − AFNI 25.2.09" on a page of two ports names the clean room).
_PORT_WORDS = {
    "clean-room": r"(?:the )?(?:LARMORX |larmorx )?(?:clean-room original|clean room|original)",
    "replica": r"(?:the )?(?:larmorx(?:-nc|-gpl)? replica|larmorx \(replica\)|replica)",
}


def _label(page: Page, old: str, pa: str = "", pb: str = "", runs: dict | None = None) -> str:
    """A comparison's label: LARMORX's port by its name (``pa``, ``pb``: the ports of its two
    runs), the original by its program. ``runs``: each panel's new name and version, for the
    labels that name a build without them ("fMRIPrep's AFNI", "the repository's AFNI")."""
    s = old
    program = page.program
    runs = runs or {}
    # a label that begins with its program ("antsAI: larmorx − ANTs, 1 thread", on a page of
    # two programs): the program goes to the original's name
    m = re.match(r"^(\w+): (.*)$", s)
    if m and m.group(1) in (page.program, *page.programs):
        program, s = m.group(1), m.group(2)
    # the ports by their own names, at either end
    if pa in _PORT_WORDS:
        s = re.sub(rf"^{_PORT_WORDS[pa]}(?![\w-])", words.PORT_NAME[pa], s)
    if pb in _PORT_WORDS:
        s = re.sub(rf" − {_PORT_WORDS[pb]}(?![\w-])", f" − {words.PORT_NAME[pb]}", s)
    who = words.PORT_NAME.get(pa) or words.PORT_NAME[page.port]
    for pat, rep in _LABEL:
        s = re.sub(pat, rep.format(port=who), s)
    sw = re.escape(page.software)
    a, b = runs.get("a") or {}, runs.get("b") or {}
    # "fMRIPrep's AFNI − the repository's AFNI build": the builds by the runs' names
    if a.get("name") and (m := re.match(rf"^fMRIPrep's (own )?{sw}\b", s)):
        s = a["name"] + s[m.end() :]  # "fMRIPrep's own AFNI": fMRIPrep's output
    if b.get("name") and "fMRIPrep's build" in b["name"]:  # "replica − fMRIPrep's AFNI"
        s = re.sub(rf" − fMRIPrep's {sw}\b", f" − {b['name']}", s)
    if b.get("name"):
        rep = b["name"] if a.get("role") == "fmriprep" else "the repository's build"
        s = re.sub(rf" − the repository's {sw}(?: build)?\b", f" − {rep}", s)
    # the first mention of the software names the program; a second one is the same program
    first = re.search(rf"\b{sw}\b", s)
    if first and program:  # (an example of several programs' calls names the software)
        head, tail = s[: first.end()], s[first.end() :]
        named = tail.startswith(f" {program}")
        tail = re.sub(rf"\b{sw} (?=\d+\.\d|on\b)", "", tail)
        s = f"{head}{tail}" if named else f"{head} {program}{tail}"
    # a mention without a version names its run's, in an example of two versions of the
    # original and on the pages that ask for it (the ANTs pages, presented before, do not)
    if first and program and (runs.get("versions", 0) > 1 or page.versions_in_labels):
        m = re.search(rf"\b{sw} {re.escape(program)}(?! [0-9])", s)
        if m:
            side = a if " − " in s[m.end() :] or " − " not in s else b
            if side.get("version"):
                s = f"{s[: m.end()]} {side['version']}{s[m.end() :]}"
    return s


_THREADS = re.compile(r"(\d+) threads?(?: [^−]*)? − (\d+) threads?")
_IDS = re.compile(r"\(((?:[A-Z]+-\d+)(?:(?:, | and )[A-Z]+-\d+)*)\)")
_VERSION = re.compile(r"\b\d+(?:\.\d+){2,}\b")


def _vkey(v: str) -> tuple[int, ...]:
    return tuple(int(x) for x in re.findall(r"\d+", v))


def _what(
    old: str, a: dict | None, b: dict | None, rl: str = "", va: str = "", vb: str = ""
) -> str | None:
    """What changes between two runs of one program, from a comparison's old label (``rl``: its
    old role label; ``va``, ``vb``: the versions of its two runs, where they are the
    original's)."""
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
    # the shapes of the other software's pages
    if a is not None and "fMRIPrep's image" in a.get("label", ""):  # "fMRIPrep's AFNI − ..."
        if va and vb and va != vb:
            return f"fMRIPrep's build of {va} and the repository's {vb}"
        return "fMRIPrep's build and the repository's"
    if m := re.search(r"(\d+) threads? − all CPUs", old):
        return f"{m.group(1)} thread{'s' if m.group(1) != '1' else ''} and all CPUs"
    if m := re.search(r"--nthr=(\d+) − --nthr=(\d+)|--n-threads (\d+) − (\d+)", old):
        n = sorted({int(x) for x in m.groups() if x})
        return f"{n[0]} and {n[1]} threads" if len(n) == 2 else None
    if rl.startswith("the original against itself: ") and not rl.endswith("the version step"):
        return rl.removeprefix("the original against itself: ")  # its own words already
    if "numpy" in old and "SciPy" in old:
        return "two releases of numpy and SciPy"
    if rl.endswith("the version step") or len(set(_VERSION.findall(old))) == 2:
        vs = sorted(set(_VERSION.findall(old)) or {va, vb} - {""}, key=_vkey)
        return f"versions {vs[0]} and {vs[1]}" if len(vs) == 2 else None
    if re.search(r"\bstored \w+ − \w+", old):
        return "the same volumes stored another way"
    if "usual encoding" in old:
        return "two encodings"
    if m := re.search(r"\bwith (?:the )?(\w+) − .*\bwithout (?:it|the \1)\b", old):
        return f"with and without the {m.group(1)}"
    if "second run − first run" in old:
        return "two runs of the same command"
    if "reversed input − unperturbed" in old:
        return "the volumes in reverse order"
    if "AVX-512 kernels on − off" in old:
        return "numpy's AVX-512 kernels on and off"
    return None


#: Old role labels that say what a comparison is where its shape does not (the guide's
#: section 4, "Today's comparison labels and the new ones"): the new role and its group.
_ROLE_LABELS = {
    "what the registration changed": (
        "the original against its starting point: what the registration changed",
        "its starting point",
    ),
    "the original against the truth": ("the original against the known answer", "the known answer"),
    "the same data, two encodings": ("the input against itself: two encodings", "itself"),
    "the two implementations of the corrections": (
        "the clean room against the replica: both fix",
        "the replica",
    ),
}


def _defect(page: Page, old: str) -> str:
    """ "AFNI: -peels 0 − -peels 1", "AFNI on the negative scale factor − AFNI on the usual
    encoding": the two runs that show an original's defect, by what differs."""
    sw = re.escape(page.software)
    sides = []
    for x in old.split(" − "):
        x = re.sub(rf"^{sw}(?: [0-9][\w.]*)?:? ", "", x.strip())
        sides.append(x.removeprefix("on "))
    if len(sides) != 2:
        raise ValueError(f"a defect's comparison of another shape: {old!r}")
    return f"the defect ({sides[0]} and {sides[1]})"


def _role(
    page: Page, d: dict, pa: str, pb: str, panels: dict, runs: dict | None = None
) -> tuple[str, str]:
    """A comparison's role label (the guide's section 4) and what it is compared with, for the
    metrics table's groups."""
    old, who = d["label"], words.PORT_NAME.get(pa)
    rl = d.get("role_label") or ""
    a, b = panels.get(d["a"]), panels.get(d["b"])
    va, vb = ((runs or {}).get(x, {}).get("version", "") for x in (d["a"], d["b"]))
    if rl in _ROLE_LABELS:
        return _ROLE_LABELS[rl]
    if rl == "against the truth" and (who or pa == "original"):
        return f"the {who or 'original'} against the known answer", "the known answer"
    if rl.startswith("the defect") and pa == pb == "original":
        return f"the original against itself: {_defect(page, old)}", "itself"
    if rl == "the corrected result" and who and pb == "original":
        return f"the {who} against the original: fix, on the usual encoding", "the original"
    if who and pb == pa:
        if rl in ("larmorx's corrections", "larmorx's correction"):
            ids = _IDS.search(old)
            what = "fix and reproduce" + (f" ({ids.group(1)})" if ids else "")
        elif "thread" in old and not re.search(r"default|reproduce|fix", old.split("−")[0]):
            what = _what(old, a, b, rl)
        elif re.search(r"\b(default|fix)\b.*− .*reproduce|reproduce.*− .*default", old):
            ids = _IDS.search(old)
            what = "fix and reproduce" + (f" ({ids.group(1)})" if ids else "")
        else:
            what = _what(old, a, b, rl)
        if not what:
            raise ValueError(f"{d['id']}: what changes in {old!r}?")
        return f"the {who} against itself: {what}", "itself"
    if pa == "clean-room" and pb == "replica":
        both = ""
        if re.search(r"\bdefault\b|\bfix\b", old) and "reproduce" not in old:
            both = ": both fix"
        elif "reproduce" in old and not re.search(r"\bdefault\b|\bfix\b", old):
            both = ": both reproduce"
        return f"the clean room against the replica{both}", "the replica"
    if who and pb in ("original", "fmriprep") and pb == "fmriprep":
        return f"the {who} against fMRIPrep's output", "fMRIPrep's output"
    if who and pb == "original":
        role = f"the {who} against the original"
        if (b is not None and b.get("role") == "reference") or (
            b is None and "correctly rounded" in old
        ):
            role += ": correctly rounded maths"
        elif b is not None and "fMRIPrep's image" in b.get("label", ""):
            role += ": fMRIPrep's build"
        elif re.search(r"\bseed\b.* − .*\bthe clock's seed\b", old):
            # "larmorx, seed 121212 − ANTs, the clock's seed (run 1)": two seeds, not one call
            role += ": a fixed seed and the clock's"
        return role, "the original"
    if who and pb == "input":
        return f"the {who} against the known answer", "the known answer"
    if pa in ("original", "fmriprep") and pb == "input":
        return "the original against the known answer", "the known answer"
    if pa in ("original", "fmriprep") and pb in ("original", "fmriprep"):
        what = _what(old, a, b, rl, va, vb)
        if not what:
            raise ValueError(f"{d['id']}: what changes in {old!r}?")
        return f"the original against itself: {what}", "itself"
    raise ValueError(f"{d['id']}: a comparison of {pa} and {pb}")


#: A run's id that names one of LARMORX's ports ("larmorx-8", "lx-1", "replica-6",
#: "original-6" for the clean room on a page of two ports): never taken for the original's.
_PORT_ID = re.compile(r"^(?:larmorx|lx|replica|original|clean)\b")


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
    known: tuple[str, ...] = (),
    errors: list[str] | None = None,
) -> dict:
    """One worked example of ``page`` in the guide's words and order. ``shows`` and
    ``verdicts`` as :func:`words.example`; ``title`` the example's new title; ``runs`` the
    port of a run a comparison names without a panel (by default: a variability comparison's
    is the original's, a check's the port of the run it is compared with); ``panels`` and
    ``diffs`` override what the shapes give; ``known``: the panels that hold the known answer
    (sdcflows' exact field, computed another way: the role ``reference`` holds it beside the
    original's runs), shown in the input's grey (the guide's section 5) and compared as "the
    known answer". ``errors``: a dry run that lists every shape it
    cannot name (each panel's, run's and comparison's) instead of stopping at the first, and
    then returns ``ex`` unchanged but for the page's own rewordings
    (``validation/investigations/docs-revision/probe_present.py``)."""

    def attempt(fn, *args, fallback=None):
        if errors is None:
            return fn(*args)
        try:
            return fn(*args)
        except ValueError as exc:
            errors.append(str(exc))
            return fallback

    def refuse(msg: str, fallback: str) -> str:
        if errors is None:
            raise ValueError(msg)
        errors.append(msg)
        return fallback

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
    main_b: dict[str, list[str]] = {}
    for d in ex["diffs"]:
        if d["kind"] == "main" and d["a"] in by_id:
            main_b.setdefault(d["a"], []).append(d["b"])
    originals, info = {}, {}
    for pid, p in by_id.items():
        if pid in known:  # the known answer: the input's port, its label kept
            if p["role"] not in ("reference", "input"):
                raise ValueError(f"{ex['id']}: the known answer {pid} is a {p['role']} panel")
            new[pid] = (p["label"], text(page, p.get("sub"), code=False))
            ports[pid] = "input"
        elif p["role"] in ("original", "reference"):
            fallback = (p["label"], p.get("sub"), 0.0)
            name, sub, r = attempt(_original_panel, page, p, fallback=fallback)
            new[pid], originals[pid], rank[pid] = (name, sub), name, r
            info[pid] = {"name": name, "role": p["role"]}
            # a run of a version as built, or fMRIPrep's build (its version may be another)
            if (r == 0.0 and name != p["label"]) or r == 1.0:
                info[pid]["version"] = attempt(_original_parts, page, p, fallback=("",) * 5)[1]
        elif p["role"] == "input":
            new[pid] = (p["label"], text(page, p.get("sub"), code=False))
        elif p["role"] == "fmriprep":
            new[pid] = _fmriprep_panel(page, p)
            originals[pid] = "fMRIPrep's output"
            info[pid] = {"name": "fMRIPrep's output", "role": "fmriprep"}
    versions = {i["version"] for k, i in info.items() if i.get("version") and rank.get(k) == 0.0}
    for pid, p in by_id.items():
        if p["role"] == "larmorx":
            ports[pid] = attempt(port_of_label, page, p["label"], fallback=page.port)
            if pid in (panels or {}):  # a panel the page names itself (its label has no mode)
                continue
            # what it is compared with: the version it follows where a panel says so, else the
            # run of its first main comparison (none where that run is another port's)
            bs = main_b.get(pid, [])
            follows = [
                b for b in bs if b in originals and re.search(
                    "|".join(pat for pat, _ in _FOLLOWED), by_id[b].get("sub") or "")
            ]  # fmt: skip
            against = originals.get((follows or bs or [""])[0])
            if against and follows:
                against += ", which it follows"
            fallback = (p["label"], p.get("sub"))
            new[pid] = attempt(_port_panel, page, p, against, ports[pid], fallback=fallback)
    new.update(panels or {})

    def port_of(pid: str) -> str:
        if pid in ports:
            return ports[pid]
        if pid in by_id:
            return words.ROLE_PORT[by_id[pid]["role"]]
        return hidden[pid]

    # each run a comparison names without a panel: its port from ``runs``, or from the run it
    # is compared with (a check's: the same port; the original's in a comparison of the
    # original's own variability, or as a main comparison's second run)
    hidden: dict[str, str] = dict(runs or {})
    for d in ex["diffs"]:
        for end, other in (("a", "b"), ("b", "a")):
            pid = d[end]
            if pid in by_id or pid in hidden:
                continue
            if d["kind"] == "variability" or (d["kind"] in ("main", "fmriprep") and end == "b"):
                if _PORT_ID.match(pid):  # a port's run is never taken for the original
                    hidden[pid] = refuse(
                        f"{ex['id']}: the port's run {pid} in a {d['kind']} comparison",
                        "original",
                    )
                    continue
                hidden[pid] = "original"
            elif d["kind"] == "check" and d[other] in by_id:
                hidden[pid] = port_of(d[other])
            else:
                hidden[pid] = refuse(f"{ex['id']}: run {pid} of {d['id']} has no port", "original")
    out, target = {}, {}
    for d in ex["diffs"]:
        pa, pb = port_of(d["a"]), port_of(d["b"])
        if d["id"] in (diffs or {}):  # the page names it (below): its shape need not be read
            continue
        role, target[d["id"]] = attempt(
            _role, page, d, pa, pb, by_id, info, fallback=("?", "the original")
        )
        runs_ = {"a": info.get(d["a"]), "b": info.get(d["b"]), "versions": len(versions)}
        out[d["id"]] = (_label(page, d["label"], pa, pb, runs_), role)
    if errors:
        return ex
    for k, v in (diffs or {}).items():
        out[k] = v
        target[k] = v[1].split(" against ", 1)[1].split(":")[0]
    # the metrics table's groups: the input and the original against themselves, then each
    # port's comparisons
    order = {p: (words.PORT_ORDER.index(port_of(p)), rank.get(p, 0.0), i)
             for i, p in enumerate(by_id)}  # fmt: skip
    targets = [
        "the original", "fMRIPrep's output", "the known answer", "its starting point", "itself",
        "the replica",
    ]  # fmt: skip

    def place(pid: str) -> tuple:
        return order.get(pid, (words.PORT_ORDER.index(port_of(pid)), 9.0, 99))

    groups = []
    side = {"input": "input", "fmriprep": "original"}
    for port in ("input", "original", "replica", "clean-room"):
        ds = [d for d in ex["diffs"] if side.get(port_of(d["a"]), port_of(d["a"])) == port]
        if not ds:
            continue
        ds.sort(key=lambda d: (targets.index(target[d["id"]]), place(d["a"]), place(d["b"])))
        if port == "input":
            whats = list(dict.fromkeys(out[d["id"]][1].split("itself: ", 1)[-1] for d in ds))
            groups.append((f"The input against itself: {', '.join(whats)}", [d["id"] for d in ds]))
        elif port == "original":
            # the original against the known answer (a phantom's truth), against its starting
            # point, then against itself
            for t, title_ in (
                ("the known answer", "The original against the known answer"),
                ("its starting point", "The original against its starting point"),
            ):
                ids = [d["id"] for d in ds if target[d["id"]] == t]
                if ids:
                    groups.append((title_, ids))
            # a defect of the original, shown by two of its runs, before its yardstick
            defect = [d["id"] for d in ds if "against itself: the defect" in out[d["id"]][1]]
            if defect:
                groups.append(("The original against itself: the defect", defect))
            rest = [
                d["id"] for d in ds if d["id"] not in defect and target[d["id"]] not in (
                    "the known answer", "its starting point")
            ]  # fmt: skip
            if rest:
                groups.append(("The original against itself: the yardstick", rest))
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
                korder = ["thread counts", "fix and reproduce", *sorted(kinds)]
                named = ", ".join(k for k in dict.fromkeys(korder) if k in kinds)
                groups.append((f"The {who} against itself: {named}", [d["id"] for d in itself]))
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
    # the port's own words in the example's texts (words.example has reworded them); on a page
    # of two ports the example's own texts are about both, each reason about its comparison's
    both = "both" if len({v for v in ports.values() if v in words.PORT_NAME}) > 1 else ""
    ex["summary"] = text(page, ex.get("summary"), port=both)
    for f in ex.get("facts", []):
        f["label"] = text(page, f["label"], code=False, port=both)
        f["text"] = text(page, f["text"], port=both)
    ex["metrics_note"] = text(page, ex.get("metrics_note"), port=both)
    for d in ex["diffs"]:
        mine = d["port"] if d["port"] in words.PORT_NAME else both
        d["reason"] = warnings(text(page, d.get("reason"), port=mine))
    ex["provenance"] = [oracle(x) for x in ex.get("provenance") or []]
    if ex.get("poster", {}).get("caption"):
        ex["poster"]["caption"] = text(page, ex["poster"]["caption"], code=False, port=both)
    if ex.get("chain"):
        chain(page, ex["chain"])
    for t in ex.get("tables") or []:
        example_table(page, t)
    return ex


def example_table(page: Page, t: dict) -> None:
    """A worked example's own table (a registration's parameters, its fields' differences):
    its title and note in the guide's words, LARMORX's run named by its port in the columns
    and rows ("larmorx − ANTs as built" -> "replica − ANTs as built")."""
    name = words.PORT_NAME[page.port]

    def run(s: str | None) -> str | None:
        if not s:
            return s
        return _mode(re.sub(r"(?<![\w./`-])larmorx(?: \(port\))?(?![\w/`-])", name, s))

    t["title"] = text(page, t.get("title"))
    t["note"] = text(page, t.get("note"))
    for c in t.get("columns", []):
        c["label"] = run(c["label"])
    for r in t["rows"]:
        r["head"] = run(r.get("head"))
        if r.get("sub"):
            r["sub"] = text(page, r["sub"])
        for c in r["cells"]:
            if "text" in c:
                c["text"] = text(page, c["text"])


def chain(page: Page, ch: dict) -> None:
    """A worked example's chain of causes (what separates fMRIPrep's output from the port's):
    each run named as the panels name it, LARMORX's by its port (``port`` on every node). The
    site shows its title and nodes as plain text."""
    ch["title"] = text(page, ch["title"], code=False)
    for step in ch["steps"]:
        for n in step["nodes"]:
            if n["role"] == "larmorx":
                m = _PORT_LABEL.match(n["label"])
                if not m or m.group("own") or m.group("prog"):
                    raise ValueError(f"a chain's run of another shape: {n['label']!r}")
                port = port_of_label(page, n["label"])
                n["label"] = words.PORT_NAME[port] + _mode(m.group("rest") or "")
                n["port"] = port
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


def cases(page: Page, cs: list[dict], ports: dict[str, str] | None = None) -> None:
    """Every case's texts in the guide's words, its command's placeholders included; each
    result's reason names its own port (``ports``: a record's implementation id -> its port, on
    a page of two)."""
    for c in cs:
        if c.get("command"):
            c["command"] = c["command"].replace(*_RUN_PLACEHOLDER)
        c["description"] = text(page, c.get("description"), code=False)
        for impl, r in c["results"].items():
            r["reason"] = text(page, r.get("reason"), port=(ports or {}).get(impl, ""))


def table(page: Page, t: dict, runs: bool = False) -> None:
    """A table's texts in the guide's words; ``runs``: its rows are a worked example's runs,
    named as its panels name them (:func:`run_rows`)."""
    t["title"] = text(page, t.get("title"))
    t["note"] = text(page, t.get("note"))
    if runs:
        run_rows(page, t)
    for r in t["rows"]:
        if not runs:
            r["head"] = text(page, r.get("head"))
            if r.get("sub"):
                r["sub"] = text(page, r["sub"])
        for c in r["cells"]:
            if "text" in c:
                c["text"] = text(page, c["text"])


def _last_bracket(s: str) -> tuple[str, str]:
    """A run's name split at its last bracket of the top level: "ANTs 2.6.5, 1 thread (as built
    (GCC 14.3), glibc's maths)" gives ("ANTs 2.6.5, 1 thread", "as built (GCC 14.3), glibc's
    maths")."""
    if not s.endswith(")"):
        raise ValueError(f"a run's name without its bracket: {s!r}")
    depth = 0
    for i in range(len(s) - 1, -1, -1):
        depth += {")": 1, "(": -1}.get(s[i], 0)
        if depth == 0:
            return s[:i].rstrip(), s[i + 1 : -1]
    raise ValueError(f"unbalanced brackets: {s!r}")


def run_rows(page: Page, t: dict) -> None:
    """A table whose rows are the runs of a worked example ("every run's transform": each
    head "<panel's label> (<its second line>)"): each run named as the panels name it, its
    second line under it, so that no bracket stands inside another."""
    for r in t["rows"]:
        label, sub = _last_bracket(r["head"])
        if re.match(r"^(?:\w+: )?larmorx\b", label):
            r["head"], r["sub"] = _port_panel(page, {"label": label, "sub": sub}, None)
        else:
            role = "reference" if "correctly rounded" in label else "original"
            name, second, _ = _original_panel(page, {"label": label, "sub": sub, "role": role})
            r["head"], r["sub"] = name, second


def dots(page: Page, s: dict | None, title: str, labels: dict[str, str]) -> None:
    """A dots chart (a thread band beside the port's distance from the original, one row per
    case): its title by the guide's pattern, each series by ``labels`` (series id -> name),
    the port's points in its mark."""
    if not s:
        return
    s["title"] = title
    for ser in s["series"]:
        ser["label"] = labels[ser["id"]]
        mark = ser["mark"]
        if mark.startswith("larmorx"):
            ser["mark"] = page.port + mark.removeprefix("larmorx")
        ser["port"] = ser["mark"].removesuffix("-open")
    s["note"] = text(page, s.get("note"))
    s["port"] = page.port


#: The marks of a line chart's series on a page in the guide's words (section 5): the original
#: solid, the replica long dashes, the clean room short dashes, ``-reproduce`` a thinner line of
#: the same port; ``band`` a limit's band (a wash of the original's ink).
LINE_MARKS = (
    "original", "replica", "replica-reproduce", "clean-room", "clean-room-reproduce", "band",
)  # fmt: skip


def lines(
    page: Page,
    spec: dict,
    title: str,
    marks: dict[str, str],
    labels: dict[str, str],
    panel_labels: list[tuple[str, str]] = (),
) -> None:
    """A line chart over one x axis (a run's motion parameters, volume by volume): its title by
    the guide's pattern, each key's and series' mark by its port (``marks``: the old mark -> one
    of :data:`LINE_MARKS`; the old "larmorx" is a port's, which the chart says), each key's and
    series' label by ``labels`` (old -> new; every label must be named), each panel's label by
    ``panel_labels`` (pattern, replacement); its note and its table of statistics in the
    guide's words (the table's rows are runs compared: their names by the page's own
    rewordings)."""
    spec["title"] = title
    for k in spec["key"]:
        k["mark"] = marks.get(k["mark"], k["mark"])
        k["label"] = labels[k["label"]]
    for p in spec["panels"]:
        for pat, rep in panel_labels:
            p["label"] = re.sub(pat, rep, p["label"])
        for ser in p["series"]:
            ser["mark"] = marks.get(ser["mark"], ser["mark"])
            ser["label"] = labels[ser["label"]]
        for b in p.get("bands") or []:
            b["mark"] = marks.get(b["mark"], b["mark"])
    used = {k["mark"] for k in spec["key"]} | {
        s["mark"] for p in spec["panels"] for s in [*p["series"], *(p.get("bands") or [])]
    }  # fmt: skip
    if bad := sorted(used - set(LINE_MARKS)):
        raise ValueError(f"a line chart's marks without a port: {bad}")
    spec["note"] = text(page, spec.get("note"))
    if spec.get("stats"):
        table(page, spec["stats"])


def original_name(page: Page, code: bool = True) -> str:
    """The original in a title: "ANTs `N4BiasFieldCorrection` 2.6.5"; a part of the software
    (:attr:`Page.part`): "ITK 5.4.5's transform files"."""
    if page.part and not page.program and not page.programs:
        return f"{page.software} {page.version}'s {page.part}"
    names = [page.program] if page.program else list(page.programs)
    if not names:
        raise ValueError("a page without its program")
    q = [f"`{n}`" if code else n for n in names]
    prog = q[0] if len(q) == 1 else ", ".join(q[:-1]) + " and " + q[-1]
    return f"{page.software} {prog} {page.version}"


def implementation(page: Page, im: dict, comparison: str, port: str = "") -> None:
    """An agreement chart's title and names (the guide's section 4); ``port``: its record's
    port, on a page of two (by default the page's)."""
    port = port or page.port
    who = words.PORT_NAME[port]
    im["title"] = f"Agreement, {who} against the original: {original_name(page)}"
    im["comparison"] = comparison
    im["label"] = f"{who} against the original: {original_name(page, code=False)} ({comparison})"
    im["short"] = who
    im["port"] = port


def speed(
    page: Page,
    sp: dict | None,
    caption: str,
    threads: str = "",
    marks: dict[str, str] | None = None,
    labels: dict[str, str] | None = None,
) -> None:
    """A speed chart's title, caption and marks: the port's points by its name. ``marks``: the
    old marks' ports where "larmorx" is not the page's port (a page of two ports, whose old
    "larmorx" mark is the clean room's); ``labels``: a series' new label by its old one."""
    if not sp:
        return
    marks = marks or {"larmorx": page.port, "larmorx-open": f"{page.port}-open"}
    ports = {m.removesuffix("-open") for m in marks.values()} | {
        ser["mark"].removesuffix("-open") for ser in sp["series"] if ser["mark"] in PORTS_MARKS
    }  # fmt: skip
    who = words.PORT_NAME[page.port]
    a = "both ports" if len(ports) > 1 else f"the {who}"
    sp["title"] = f"Speed, {a} against the original: {original_name(page)}{threads}"
    sp["caption"] = caption
    for ser in sp["series"]:
        ser["mark"] = marks.get(ser["mark"], ser["mark"])
        ser["port"] = ser["mark"].removesuffix("-open")
        if labels and ser["label"] in labels:
            ser["label"] = labels[ser["label"]]
            continue
        lab = re.sub(r"^larmorx\b", who, ser["label"])
        if page.program:
            lab = re.sub(
                rf"^{re.escape(page.software)}(?=,)", f"{page.software} {page.program}", lab
            )
        ser["label"] = lab
    sp["note"] = text(page, sp.get("note"))


#: The marks of the ports' points (open: at several threads).
PORTS_MARKS = ("replica", "replica-open", "clean-room", "clean-room-open")


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


def finish(
    page: Page,
    doc: dict,
    labels: dict[str, str] | None = None,
    runs: tuple[str, ...] = (),
    pages: dict[str, Page] | None = None,
    ports: dict[str, str] | None = None,
) -> dict:
    """The page's tables (``runs``: the ids of those whose rows are a worked example's runs;
    ``pages``: a table's own page, where its runs are of another program of the page), cases,
    its categories' names and the mark that it is presented."""
    for t in doc.get("tables", []):
        table((pages or {}).get(t["id"], page), t, runs=t["id"] in runs)
    cases(page, doc.get("cases", []), ports)
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


def _panel_facts(page: Page, ex: dict, port: str, ports: dict[str, str]) -> dict | None:
    """What a data-panel example shows of one port, by what each comparison is: the port
    against the original's run of the page's version as built (its main comparison), against
    correctly rounded maths, against itself by threads, and ``fix`` against ``reproduce``."""
    by_id = {p["id"]: p for p in ex["panels"]}
    mine = {pid for pid, x in ports.items() if x == port}
    if not mine:
        return None

    def built(pid: str) -> bool:  # the original's run of the page's version, as built
        p = by_id.get(pid)
        if not p or p["role"] != "original":
            return False
        try:
            _, ver, _, _, rank = _original_parts(page, p)
        except ValueError:
            return False
        return rank == 0.0 and ver == page.version

    def same(d: dict) -> bool:  # two runs of this port (one may have no panel)
        return (d["a"] in mine or d["a"] not in by_id) and (d["b"] in mine or d["b"] not in by_id)

    main = next(
        (d for d in ex["diffs"] if d["kind"] == "main" and d["a"] in mine and built(d["b"])), None
    )
    if main is None:
        raise ValueError(
            f"{ex['id']}: no comparison of the {words.PORT_NAME[port]} with the original as built"
        )
    cr = next(
        (d for d in ex["diffs"] if d["a"] == main["a"] and d is not main and (
            by_id.get(d["b"], {}).get("role") == "reference"
            or (d["b"] not in by_id and "correctly rounded" in d["label"]))), None,
    )  # fmt: skip
    threads = [
        d for d in ex["diffs"] if same(d) and main["a"] in (d["a"], d["b"])
        and "thread" in d["label"]
        and not re.search(r"default|reproduce|fix", d["label"].split("−")[0])
    ]  # fmt: skip
    fix = next(
        (d for d in ex["diffs"] if same(d) and d["a"] in mine and d["b"] in mine and re.search(
            r"\b(default|fix)\b.*− .*reproduce", d["label"])), None,
    )  # fmt: skip
    n = []
    if threads and threads[0]["verdict"] == "bit-identical":
        n = sorted({int(x) for x in re.findall(r"(\d+) threads?", threads[0]["label"])})
    mode = "reproduce" if "reproduce" in (by_id[main["a"]].get("sub") or "") else "fix"
    return {"main": main, "cr": cr, "threads": n if len(n) == 2 else [], "fix": fix, "mode": mode}


def _fix_clause(ex: dict, fix: dict | None, whose: str, port: str = "") -> str:
    """What ``fix`` changes on the call, from the comparison of ``fix`` with ``reproduce``
    (``port``: the port it is said of, on a page of two)."""
    if fix is None:
        return ""
    if fix["verdict"] == "bit-identical":
        if port:
            return f"No correction by default changes the {port}'s result on this call."
        return "No correction by default changes this call."
    if fix["verdict"] == "d13":
        # "corrects" only for the corrections that change values on this call (WARNS)
        ids = list(dict.fromkeys(re.findall(r"\b[A-Z]+-\d+\b", fix.get("reason") or "")))
        fixed = [i for i in ids if i not in WARNS]
        if not fixed:
            raise ValueError(f"{ex['id']}: {fix['id']} changes values, but names only {ids}")
        line = f"{whose} default, `fix`, corrects {' and '.join(fixed)} here"
        warned = [f"{WARNS[i]} ({i})" for i in ids if i in WARNS]
        return line + "".join(f" and {w}" for w in warned) + "."
    raise ValueError(f"{ex['id']}: {fix['id']} is {fix['verdict']}")


def panel_shows(page: Page, ex: dict) -> tuple[str, list[str]]:
    """The "What this shows" line of a data-panel example, from its own verdicts, metrics and
    reasons, each comparison found by what it is (:func:`_panel_facts`), not by its id; on a
    page of two ports one line for both; and the verdicts that stand beside it, one per port.
    It serves the data panel's calls of fMRIPrep, where a port's run is compared with the
    original's run as built and, where the page has one, with correctly rounded maths."""
    ports = {
        p["id"]: port_of_label(page, p["label"]) for p in ex["panels"] if p["role"] == "larmorx"
    }
    facts = {
        port: f for port in ("replica", "clean-room")
        if (f := _panel_facts(page, ex, port, ports)) is not None
    }  # fmt: skip
    if not facts:
        raise ValueError(f"{ex['id']}: no port's panel")
    sw = _owner(page.software)

    def gives(port: str, f: dict, alone: bool) -> str:
        who = words.PORT_NAME[port]
        main = f["main"]
        head = "In `reproduce`" if f["mode"] == "reproduce" else "In `fix`, the default"
        if alone and f["threads"]:
            head += f", on {f['threads'][0]} and {f['threads'][1]} threads"
        if main["verdict"] == "bit-identical":
            if f["cr"] is not None and f["cr"]["verdict"] != "bit-identical":
                raise ValueError(f"{ex['id']}: {main['id']} bit-identical, {f['cr']['id']} not")
            how = ", as built and with correctly rounded maths" if f["cr"] is not None else ""
            return f"{head}, the {who} gives {sw} bytes{how}."
        if main["verdict"] == "identical-cr":
            big = max(m["max_abs"] for m in main["metrics"].values())
            return (
                f"{head}, the {who} gives {sw} bytes with correctly rounded maths; "
                f"{page.software} as built differs only where glibc's maths is not correctly "
                f"rounded (largest difference {_sig(big)})."
            )
        raise ValueError(f"{ex['id']}: {main['id']} is {main['verdict']}")

    if len(facts) == 1:
        port, f = next(iter(facts.items()))
        parts = [gives(port, f, True), _fix_clause(ex, f["fix"], "Its")]
        return " ".join(x for x in parts if x), [f["main"]["id"]]
    fr, fc = facts["replica"], facts["clean-room"]
    verdicts = [fr["main"]["id"], fc["main"]["id"]]
    alike = (
        fr["main"]["verdict"] == fc["main"]["verdict"] == "bit-identical"
        and fr["mode"] == fc["mode"] and (fr["cr"] is None) == (fc["cr"] is None)
    )  # fmt: skip
    if not alike:
        line = " ".join([gives("replica", fr, True), gives("clean-room", fc, True)])
    else:
        head = "In `reproduce`" if fr["mode"] == "reproduce" else "In `fix`, the default"
        how = ", as built and with correctly rounded maths" if fr["cr"] is not None else ""
        threads = {p: f["threads"] for p, f in facts.items() if f["threads"]}
        note = ""
        if len(threads) == 2 and threads["replica"] == threads["clean-room"]:
            head += f", on {threads['replica'][0]} and {threads['replica'][1]} threads"
        elif threads:
            note = " (" + "; ".join(
                f"the {words.PORT_NAME[p]} on {n[0]} and {n[1]} threads" for p, n in threads.items()
            ) + ")"  # fmt: skip
        line = f"{head}, the replica and the clean room both give {sw} bytes{how}{note}."
    fixes = [fr["fix"], fc["fix"]]
    if all(x is not None and x["verdict"] == "bit-identical" for x in fixes):
        clause = "No correction by default changes this call, in either port."
    else:
        clause = " ".join(
            c for c in (_fix_clause(ex, fr["fix"], "The replica's", "replica"),
                        _fix_clause(ex, fc["fix"], "The clean room's", "clean room")) if c
        )  # fmt: skip
    return " ".join(x for x in (line, clause) if x), verdicts
