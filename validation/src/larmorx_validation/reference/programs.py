# SPDX-License-Identifier: Apache-2.0
"""The documentation site's Programs page: every program and module LARMORX ports, grouped by the
software it comes from, with its ports and their status (docs/writing-guide.md §6).

``validation/programs.toml`` says what the code and the records cannot: each group's name and
original, each program's group, one sentence of what it does, its API page and its evidence
page. The rest comes from the code and the records: the ports of a command from the replica
registry (:func:`larmorx_validation.reference.code.ports`), each port's status from its records
(``validation/tools.toml``), a module's status without a record from its API page, the reference
page from the site's checkout. The result is the site's ``data/programs.yml``, which
``content/programs/index.md`` lays out.

It reads only files a clean-room implementer may read (the writing guide, §1): the main
package's command-line code (``crates/larmorx-cli``, the clean rooms' and ANTs' ``cli`` modules),
the Python package, the API pages, ``validation/tools.toml`` and ``validation/programs.toml``.

The checks (every one a problem that stops the export):

- every command of ``larmorx`` (``larmorx_cli::FAMILIES``) has an entry, and no entry names a
  command that does not exist (``mri hmc`` is the clean room of ``fsl mcflirt``, its ``also``);
- the ports a record names are the ones the replica registry lists (or the entry's ``ports``,
  for a program the registry does not list);
- every record, API page and Python name exists; with a site, every evidence page too;
- every API page of ``docs/api/`` but the register's (``upstream-defects.md``) is some entry's
  ``api``, so a new module or API page without an entry is caught;
- what a program does has one source: where its API page holds the program's reference block
  (:mod:`larmorx_validation.reference.apipage`), the first sentence of its "what it does" text,
  and the entry gives no ``does`` of its own; elsewhere the entry's ``does``.
"""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

from larmorx_validation.reference import apipage
from larmorx_validation.reference.code import ports as registry_ports

#: How a port is named, and its package and licence where the registry does not say.
PORT_NAMES = {"replica": "replica", "clean-room": "clean room", "own": "LARMORX's own"}
#: Licences in plain words (docs/writing-guide.md §2, rule 10).
LICENCE_WORDS = {
    "Apache-2.0 AND CC-BY-4.0": "Apache-2.0, with AFNI's CC BY 4.0 credit",
    "GNU GPL version 3": "GPL-3.0",
}
OUT = "data/programs.yml"
#: API pages that describe no program or module (Validation links the register).
NOT_PROGRAMS = {"docs/api/upstream-defects.md"}


@dataclass
class Port:
    kind: str
    package: str
    licence: str
    status: str
    next_release: bool = False


@dataclass
class Program:
    group: str
    name: str
    kind: str  # "program" or "module"
    command: str
    also: str
    python: str
    does: str
    api: str
    evidence: str
    reference: str
    original: str
    ports: list[Port] = field(default_factory=list)


def _str_list(text: str, name: str) -> list[str] | None:
    m = re.search(rf"const {name}: &\[&str\] = &\[(.*?)\];", text, re.S)
    return re.findall(r'"([^"]+)"', m.group(1)) if m else None


def commands(repo: Path) -> list[tuple[str, str]]:
    """Every ``(family, tool)`` the ``larmorx`` command runs, from ``larmorx_cli::FAMILIES``."""
    lib = (repo / "crates/larmorx-cli/src/lib.rs").read_text(encoding="utf-8")
    m = re.search(r"pub const FAMILIES: &\[\(&str, &\[&str\]\)\] = &\[(.*?)\];", lib, re.S)
    out = []
    for family, ref in re.findall(r'\("(\w+)",\s*([\w:]+)\)', m.group(1)):
        if "::" in ref:
            crate = ref.split("::")[0].replace("_", "-")
            src = repo / "crates" / crate / "src"
            files = [src / "cli/mod.rs", src / "cli.rs"]
            tools = None
            for f in files:
                if f.is_file():
                    tools = _str_list(f.read_text(encoding="utf-8"), "TOOLS")
                    if tools is not None:
                        break
        else:
            tools = _str_list(lib, ref)
        if tools is None:
            raise SystemExit(f"the tools of {family} ({ref}) not found")
        out += [(family, t) for t in tools]
    return out


def _python_exists(repo: Path, dotted: str) -> bool:
    parts = dotted.split(".")
    if parts[0] != "lx":
        return False
    base = repo / "python/larmorx"
    *mods, name = parts[1:]
    pkg = base.joinpath(*mods)
    if pkg.is_dir() and (pkg / name / "__init__.py").is_file():
        return True
    if (pkg / f"{name}.py").is_file():
        return True
    init = pkg / "__init__.py"
    return (
        init.is_file() and re.search(rf"\b{re.escape(name)}\b", init.read_text("utf-8")) is not None
    )


def _api_status(repo: Path, api: str) -> str:
    m = re.search(r"\*\*Status: `?(\w+)`?\*\*", (repo / api).read_text(encoding="utf-8"))
    return m.group(1) if m else ""


def _status(record: dict | None) -> str:
    m = re.match(r"`(\w+)`", (record or {}).get("status", ""))
    return m.group(1) if m else ""


def first_sentence(text: str) -> str:
    """The first sentence of a paragraph of Markdown (its lines joined; a full stop that ends a
    sentence is followed by a blank or the text's end)."""
    flat = " ".join(text.split())
    m = re.match(r"(.+?\.)(?:\s|$)", flat)
    return m.group(1) if m else flat


def _api_does(repo: Path, api: str) -> dict[str, str]:
    """What each program with a reference block on the API page ``api`` does, by its name."""
    if not (repo / api).is_file():
        return {}
    page = apipage.read(repo, api)
    return {p.name: first_sentence(p.description.split("\n\n")[0]) for p in page.programs}


def _page_exists(site: Path, url: str) -> bool:
    rel = url.strip("/")
    return (site / "content" / f"{rel}.md").is_file() or (
        site / "content" / rel / "index.md"
    ).is_file()


def build(repo: Path, site: Path | None) -> tuple[dict, list[Program], list[str]]:
    """The groups, the programs and the problems."""
    desc = tomllib.loads((repo / "validation/programs.toml").read_text(encoding="utf-8"))
    tools = tomllib.loads((repo / "validation/tools.toml").read_text(encoding="utf-8"))
    records = {r["id"]: r for r in tools.get("record", [])}
    groups = {g["id"]: g for g in desc["group"]}
    problems: list[str] = []
    progs: list[Program] = []
    known = set(commands(repo))
    named: set[tuple[str, str]] = set()
    api_does: dict[str, dict[str, str]] = {}
    for e in desc["program"]:
        where = f"validation/programs.toml: {e['group']} {e['name']}"
        if e["group"] not in groups:
            problems.append(f"{where}: no group {e['group']!r}")
            continue
        cmd = e.get("command", "")
        reg = None
        for c in [cmd, e.get("also", "")]:
            if not c:
                continue
            words = c.split()
            key = (words[1], words[2]) if len(words) == 3 and words[0] == "larmorx" else None
            if key not in known:
                problems.append(f"{where}: `{c}` is not a command of larmorx")
            else:
                named.add(key)
        if cmd:
            _, fam, tool = cmd.split()
            reg = registry_ports(repo, fam, tool)
        rec_ports: dict = e.get("records", {})
        for ids in rec_ports.values():
            for rid in ids:
                if rid not in records:
                    problems.append(f"{where}: no record {rid!r} in validation/tools.toml")
        kinds = [p.kind for p in reg] if reg else list(e.get("ports", rec_ports.keys()))
        if reg and e.get("ports"):
            problems.append(f"{where}: `ports` given, but the replica registry lists this command")
        if sorted(kinds) != sorted(rec_ports):
            problems.append(
                f"{where}: the records name the ports {sorted(rec_ports)}, the code {sorted(kinds)}"
            )
        for path in [e["api"]]:
            if not (repo / path).is_file():
                problems.append(f"{where}: no {path}")
        if e["api"] not in api_does:
            api_does[e["api"]] = _api_does(repo, e["api"])
        from_api = api_does[e["api"]].get(e["name"], "")
        if from_api and e.get("does"):
            problems.append(
                f"{where}: `does` given, but {e['api']} says what it does (its reference block)"
            )
        does = from_api or " ".join(e.get("does", "").split())
        if not does:
            problems.append(f"{where}: no `does`, and {e['api']} has no reference block for it")
        if not _python_exists(repo, e["python"]):
            problems.append(f"{where}: no Python name {e['python']}")
        evidence = e.get("evidence", "")
        if evidence and site is not None and not _page_exists(site, evidence):
            problems.append(f"{where}: the site has no page {evidence}")
        ref = f"/programs/{e['group']}/{e['name']}/"
        reference = ref if site is not None and _page_exists(site, ref) else ""
        api_status = _api_status(repo, e["api"]) if (repo / e["api"]).is_file() else ""
        out_ports = []
        next_release = set(e.get("next_release", []))
        for kind in kinds:
            ids = rec_ports.get(kind, [])
            status = _status(records.get(ids[0])) if ids else api_status
            if not status:
                problems.append(f"{where}: no status for its {kind}")
            if reg:
                p = next(x for x in reg if x.kind == kind)
                package, licence = p.package, p.licence
            else:
                package, licence = "larmorx", "Apache-2.0"
            out_ports.append(
                Port(
                    kind, package, LICENCE_WORDS.get(licence, licence), status, kind in next_release
                )
            )
        progs.append(
            Program(
                group=e["group"],
                name=e["name"],
                kind="program" if cmd else "module",
                command=cmd,
                also=e.get("also", ""),
                python=e["python"],
                does=does,
                api=e["api"],
                evidence=evidence,
                reference=reference,
                original=e.get("original", groups[e["group"]].get("original", "")),
                ports=out_ports,
            )
        )
    for fam, tool in sorted(known - named):
        problems.append(f"validation/programs.toml: no entry for `larmorx {fam} {tool}`")
    pages = {f"docs/api/{f.name}" for f in (repo / "docs/api").glob("*.md")}
    for page in sorted(pages - NOT_PROGRAMS - {e["api"] for e in desc["program"]}):
        problems.append(f"validation/programs.toml: no entry names {page} as its `api`")
    return groups, progs, problems


def _y(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _count(n: int, one: str, many: str) -> str:
    return f"{n} {one if n == 1 else many}"


def render(groups: dict, progs: list[Program]) -> str:
    """``data/programs.yml``: deterministic, from its sources alone."""
    lines = [
        "# The Programs page's data (content/programs/index.md), generated by the main repository:",
        "#     python -m larmorx_validation reference programs --site <this checkout>",
        "# from its validation/programs.toml, the code (the commands, the replica registry) and the",
        "# records' status (validation/tools.toml). Never edit it here.",
        "groups:",
    ]
    for gid, g in groups.items():
        mine = [p for p in progs if p.group == gid]
        n_prog = sum(p.kind == "program" for p in mine)
        n_mod = len(mine) - n_prog
        parts = []
        if n_prog:
            parts.append(_count(n_prog, "program", "programs"))
        if n_mod:
            parts.append(_count(n_mod, "module", "modules"))
        kinds: dict[tuple, int] = {}
        later: dict[tuple, bool] = {}
        for p in mine:
            for x in p.ports:
                key = (x.kind, x.package, x.licence)
                kinds[key] = kinds.get(key, 0) + 1
                later[key] = later.get(key, True) and x.next_release
        lines += [
            f"  - id: {gid}",
            f"    name: {_y(g['name'])}",
            f"    original: {_y(g.get('original', ''))}",
            f"    count: {_y(' and '.join(parts))}",
        ]
        if g.get("summary"):
            lines.append(f"    summary: {_y(' '.join(g['summary'].split()))}")
        if g.get("planned"):
            lines.append(f"    planned: {_y(' '.join(g['planned'].split()))}")
        lines.append("    ports:" + ("" if kinds else " []"))
        order = ["replica", "clean-room", "own"]
        for (kind, package, licence), n in sorted(
            kinds.items(), key=lambda kv: order.index(kv[0][0])
        ):
            nr = "true" if later[(kind, package, licence)] else "false"
            lines.append(
                f"      - {{kind: {kind}, name: {_y(PORT_NAMES[kind])}, package: {package}, "
                f"licence: {_y(licence)}, count: {n}, next_release: {nr}}}"
            )
        lines.append("    programs:" + ("" if mine else " []"))
        for p in mine:
            lines += [
                f"      - name: {_y(p.name)}",
                f"        kind: {p.kind}",
                f"        command: {_y(p.command)}",
                f"        also: {_y(p.also)}",
                f"        python: {_y(p.python)}",
                f"        does: {_y(p.does)}",
                f"        original: {_y(p.original)}",
                f"        api: {_y(p.api)}",
                f"        evidence: {_y(p.evidence)}",
                f"        reference: {_y(p.reference)}",
                "        ports:",
            ]
            for x in p.ports:
                lines.append(
                    f"          - {{kind: {x.kind}, name: {_y(PORT_NAMES[x.kind])}, package: {x.package}, "
                    f"status: {_y(x.status)}, next_release: {'true' if x.next_release else 'false'}}}"
                )
    return "\n".join(lines) + "\n"


def main(repo: Path, site: Path | None, check: bool) -> int:
    groups, progs, problems = build(repo, site)
    for x in problems:
        print(f"PROBLEM: {x}")
    stale = False
    if site is not None and not problems:
        text = render(groups, progs)
        out = site / OUT
        old = out.read_text(encoding="utf-8") if out.is_file() else None
        if old != text:
            if check:
                stale = True
                print(f"STALE: {out} differs from its export")
            else:
                out.write_text(text, encoding="utf-8")
                print(f"wrote {out}")
    print(f"{len(progs)} programs and modules in {len(groups)} groups; {len(problems)} problems")
    return 1 if problems or stale else 0
