# SPDX-License-Identifier: Apache-2.0
"""Credits: the data every example and every case shows, credited from larmorx-testdata's
catalog (each collection's title, version, DOI, licence and citation), never typed by hand.

:func:`attach` runs on every tool's evidence after its export: it finds the datasets an
example names (an OpenNeuro accession ``dsNNNNNN``, a TemplateFlow template), gives the example
the ids of their credits (``credits``) and the page their credit lines (``doc["credits"]``), does
the same for the case explorer (``cases_credits``), and refuses an example whose stated licence
(its *Data* fact) is not its dataset's licence in the catalog. The site prints each credit line
under the example and under the cases (``tools/evidence.py``): for CC-BY data a proper
attribution with the changes made, for CC-BY-SA data also the share-alike notice for the images
adapted from it.
"""

from __future__ import annotations

import json
import re
from functools import cache

ACCESSION = re.compile(r"\b(ds\d{6})\b")
#: Names that point at a template collection.
TEMPLATES = {
    "MNI ICBM 152": "templateflow-MNI152NLin2009cAsym",
    "MNI152NLin2009cAsym": "templateflow-MNI152NLin2009cAsym",
    "OASIS30ANTs": "templateflow-OASIS30ANTs",
    "OASIS30": "templateflow-OASIS30ANTs",
}
#: SPDX id (or the catalog's short name) -> (name, link)
LICENCES = {
    "CC0-1.0": ("CC0 1.0 public-domain dedication", "https://creativecommons.org/publicdomain/zero/1.0/"),
    "PDDL-1.0": ("Open Data Commons Public Domain Dedication and Licence 1.0", "https://opendatacommons.org/licenses/pddl/1-0/"),
    "CC-BY-4.0": ("Creative Commons Attribution 4.0 (CC BY 4.0)", "https://creativecommons.org/licenses/by/4.0/"),
    "CC-BY-SA": ("Creative Commons Attribution-ShareAlike (CC BY-SA; the dataset gives no version)", "https://creativecommons.org/licenses/by-sa/4.0/"),
    "MNI-ICBM152": ("the MNI ICBM 152 copyright notice", "/static/hero/LICENSE-MNI.txt"),
    "Apache-2.0": ("Apache License 2.0", "https://www.apache.org/licenses/LICENSE-2.0"),
}  # fmt: skip
#: What every image of the site does to the data it shows (CC-BY: "indicate if changes were made")
CHANGES = (
    "reoriented to RAS, cropped to the region shown, masked outside it, and shown as the "
    "outputs of the programs compared"
)
STATED = re.compile(r"\b(CC0-1\.0|PDDL-1\.0|CC[- ]BY[- ]SA|CC[- ]BY[- ]?4\.0)\b")


def _norm(licence: str) -> str:
    return re.sub(r"[^a-z0-9]", "", licence.lower()).replace("ccby40", "ccby4")


@cache
def _catalog():
    import larmorx_testdata as td

    return td.catalog()


def collections_in(text: str) -> list[str]:
    """The catalog collections a text names, in the order it first names them."""
    cat = _catalog()
    found: list[str] = []
    for m in ACCESSION.finditer(text):
        cid = f"openneuro-{m.group(1)}"
        if cid in cat.collections and cid not in found:
            found.append(cid)
    for name, cid in TEMPLATES.items():
        if name in text and cid in cat.collections and cid not in found:
            found.append(cid)
    return found


def credit(cid: str) -> dict:
    """One collection's credit line, from the catalog."""
    c = _catalog().collections[cid]
    version = c.version.split(" (")[0]
    doi = getattr(c, "doi", "") or ""
    if not doi:
        m = re.search(r"doi:(10\.18112/openneuro\.[\w.]+)", c.citation)
        doi = m.group(1).rstrip(".") if m else ""
    name, link = LICENCES.get(c.licence, (c.licence, ""))
    title = re.sub(r"\s*\((OpenNeuro|TemplateFlow)[^)]*\)\s*$", "", c.title)
    where = (
        f"OpenNeuro {cid.removeprefix('openneuro-')}"
        if cid.startswith("openneuro-")
        else "TemplateFlow"
    )
    lic = c.licence
    attribution = lic.startswith("CC-BY")
    out = {
        "id": cid,
        "title": title,
        "source": where,
        "version": version,
        "doi": doi,
        "url": c.source_url,
        "licence": lic,
        "licence_name": name,
        "licence_url": link,
        "citation": c.citation,
        "attribution": attribution,
        "share_alike": lic.startswith("CC-BY-SA"),
    }
    text = f"{title} ({where}, version {version}" + (f", doi:{doi}" if doi else "") + f"), {name}."
    if attribution:
        text += f" Attribution: {c.citation} The images shown here are adapted from it: {CHANGES}."
    if out["share_alike"]:
        text += (
            " Under its share-alike condition the images adapted from this dataset are shared "
            "under the same licence, CC BY-SA."
        )
    if lic == "MNI-ICBM152":
        text += " Copyright (C) 1993–2004 Louis Collins, McConnell Brain Imaging Centre, Montreal Neurological Institute, McGill University."
    out["text"] = text
    return out


def attach(tool: str, doc: dict) -> dict:
    """Credits for a tool's evidence (in place): ``doc["credits"]`` (id -> credit), each
    example's ``credits`` (ids) and ``doc["cases_credits"]`` (ids). Raises when an example
    states a licence its datasets do not have."""
    page: dict[str, dict] = {}
    for ex in doc.get("examples", []):
        ids = collections_in(json.dumps(ex, ensure_ascii=False))
        ex["credits"] = ids
        for cid in ids:
            page.setdefault(cid, credit(cid))
        licences = {_norm(page[c]["licence"]) for c in ids}
        for fact in ex.get("facts", []):
            if fact.get("label") != "Data":
                continue
            for stated in STATED.findall(fact["text"]):
                if _norm(stated) not in licences:
                    raise ValueError(
                        f"{tool}/{ex['id']}: the Data fact states {stated!r}, the catalog says "
                        f"{sorted(page[c]['licence'] for c in ids)}"
                    )
    rest = {k: v for k, v in doc.items() if k not in ("examples", "credits", "cases_credits")}
    cases = collections_in(json.dumps(rest, ensure_ascii=False))
    for cid in cases:
        page.setdefault(cid, credit(cid))
    if cases:
        doc["cases_credits"] = cases
    doc["credits"] = dict(sorted(page.items()))
    return doc


__all__ = ["attach", "collections_in", "credit"]
