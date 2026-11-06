# SPDX-License-Identifier: Apache-2.0
"""The clean-room audit's code similarity (``cleanroom-audit similarity``), on small synthetic
inputs: the three lexers, test modules and selected functions, the two streams (a C function and
its Rust translation alike, independent code not), the measures (coverage, overlap, the longest
run, never across files), the file tables, and a whole run on a synthetic plan."""

from __future__ import annotations

import numpy as np
import pytest
from larmorx_validation.cleanroom_audit import similarity as sm

C_SNIPPET = r"""
#include <math.h>
#define SQR(x) ((x)*(x))
/* the sum of the positive squares */
static double total(const double *a, int n) {
  double s = 0.0;   // start
  for (int i = 0; i < n; i++) {
    if (a[i] > 0.5) s += (double)a[i] * SQR(a[i]);
    else s -= this->a[i].v;
    ++count;
  }
  return double(s);
}
"""
RUST_SNIPPET = r"""
/// The sum of the positive squares.
pub fn total(a: &[f64], n: usize) -> f64 {
    let mut s = 0.0_f64; /* start /* nested */ */
    for i in 0..n {
        if a[i] > 0.5 { s += a[i] as f64 * sqr(a[i]); } else { s -= self.a[i].v; }
        count += 1;
    }
    s
}
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(total(&[1.0], 1), 1.0); }
}
"""
INDEPENDENT_RUST = r"""
pub fn positive_energy(values: &[f64]) -> f64 {
    values.iter().filter(|v| **v > 0.5).map(|v| v.powi(3)).sum::<f64>()
}
"""


def stream(text: str, lang: str, mode: str) -> list[str]:
    raw = sm.lex(text, lang)
    if lang == "rust":
        raw = sm.drop_tests(raw)
    return [t for t, _ in sm.normalise(raw, lang, mode)]


# --- lexing ----------------------------------------------------------------------------------------


def test_lex_rust_comments_strings_chars_lifetimes_attributes_numbers():
    text = "a /* x /* y */ z */ b\n#[inline(always)]\nlet c = r#\"q\"#; 'x' 'a 1.5e3_f64 0..n"
    toks = sm.lex_rust(text)
    kinds = [(k, t) for k, t, _ in toks]
    assert ("id", "a") in kinds and ("id", "b") in kinds
    assert ("id", "z") not in kinds and ("id", "y") not in kinds  # nested comment
    assert ("attr", "#[inline(always)]") in kinds
    assert [k for k, t, _ in toks].count("str") == 2  # the raw string and the character
    assert ("life", "'a") in kinds
    assert ("num", "1.5e3_f64") in kinds
    assert [t for _, t in kinds[-3:]] == ["0", "..", "n"]
    assert toks[-1][2] == 3  # lines counted


def test_lex_c_preprocessor_define_comments_strings():
    text = (
        '#include <x.h>\n#define K(a) (a+1)\nint f() { /* c */ return K(2); } // d\n"s\\"t" \'q\''
    )
    toks = sm.lex_c(text)
    texts = [t for _, t, _ in toks]
    assert "include" not in texts and "x" not in texts
    assert texts[:3] == ["K", "(", "a"]  # the define's name and body kept
    assert "c" not in texts and "d" not in texts
    assert [k for k, _, _ in toks].count("str") == 2
    assert toks[-1][2] == 4


def test_lex_hash_scripts():
    toks = sm.lex_hash('#!/bin/sh\n# note\nif [ $# -lt 2 ] ; then echo "a b" ; fi\n')
    texts = [t for _, t, _ in toks]
    assert "note" not in texts and "$#" in texts and "echo" in texts


@pytest.mark.parametrize(
    ("path", "head", "lang"),
    [
        ("a/b.rs", "", "rust"),
        ("a/b.cpp", "", "c"),
        ("a/b.h", "", "c"),
        ("bet", "#!/bin/bash -e", "hash"),
        ("aff2rigid", "#!/usr/bin/env fslpython", "hash"),
        ("flirtsch/bbr.sch", "", None),
        ("README.md", "", None),
        ("AFNI_atlas_spaces.niml", "", None),
    ],
)
def test_language(path, head, lang):
    assert sm.language(path, head) == lang


def test_drop_tests_and_select_fns():
    raw = sm.drop_tests(sm.lex_rust(RUST_SNIPPET))
    assert "assert_eq" not in [t for _, t, _ in raw]
    two = "fn keep(x: f64) -> f64 { x + 1.0 }\nfn other() { y(); }\nfn also() { z() }"
    picked = [t for _, t, _ in sm.select_fns(sm.lex_rust(two), {"keep", "also"})]
    assert "keep" in picked and "also" in picked and "other" not in picked and "y" not in picked


# --- the streams -----------------------------------------------------------------------------------


def test_a_translation_keeps_its_names_stream():
    c = stream(C_SNIPPET, "c", "names")
    r = stream(RUST_SNIPPET, "rust", "names")
    c_body, r_body = c[c.index("n:s") :], r[r.index("n:s") :]
    assert c_body[-3:] == ["return", "n:s", ";"] and r_body[-1:] == ["n:s"]  # the tails
    assert c_body[:-3] == r_body[:-1]
    assert "for" in c and ".." in c  # the counting loop as Rust writes it
    assert "n:this" not in c and "n:self" not in r  # self and this dropped
    assert "n:double" not in c and "n:f64" not in r  # primitive types dropped


def test_shape_stream_collapses_chains_and_literal_lists():
    toks = stream("let v = a.b::c(x); let t = [1.0, -2.0, 3.0];", "rust", "shape")
    assert toks == ["I", "=", "I", "(", "I", ")", ";", "I", "=", "[", "N", "]", ";"]


@pytest.mark.parametrize(
    ("text", "lang"),
    [
        ("y = (double)x;", "c"),
        ("y = static_cast<double>(x);", "c"),
        ("y = double(x);", "c"),
        ("let y = x as f64;", "rust"),
        ("let y = &x;", "rust"),
        ("y = *x;", "c"),
    ],
)
def test_casts_and_unary_references_dropped(text, lang):
    assert stream(text, lang, "names") == ["n:y", "=", "n:x", ";"]


def test_c_conditions_and_increments():
    assert stream("if (a < b) c++;", "c", "names") == [
        "if",
        "n:a",
        "<",
        "n:b",
        "n:c",
        "+=",
        "N",
        ";",
    ]
    assert stream("while (k) --k;", "c", "names") == ["while", "n:k", "n:k", "-=", "N", ";"]
    assert stream("for (i = 0; i <= n; ++i) {}", "c", "names") == ["for", "n:i", "N", "..=", "n:n"]
    # not a counting loop: left as written
    assert "(" in stream("for (p = h; p; p = p->next) {}", "c", "names")


def test_names_ignore_case_and_underscores():
    assert sm.name_of("computeCost") == sm.name_of("compute_cost") == "n:computecost"


# --- the measures ----------------------------------------------------------------------------------


def corpus(*files: list[int]) -> sm.Corpus:
    parts = [
        (f"f{k}", np.asarray(f, np.int32), np.arange(1, len(f) + 1, dtype=np.int32))
        for k, f in enumerate(files)
    ]
    return sm.Corpus.build("c", parts)


def test_coverage_overlap_and_longest_run():
    a = corpus([1, 2, 3, 4, 5, 6, 7, 8, 9])
    b = corpus([20, 3, 4, 5, 6, 7, 8, 21])
    cov, per_file = sm.coverage(a, b, 4)
    assert cov == pytest.approx(6 / 9) and per_file.tolist() == [6]
    assert sm.coverage(a, b, 7)[0] == 0.0
    cont, jac = sm.overlap(a, b, 4)
    assert cont == pytest.approx(3 / 6) and jac == pytest.approx(3 / (6 + 5 - 3))
    lr = sm.longest_run(a, b)
    assert lr["tokens"] == 6
    assert lr["ours"] == {"file": "f0", "lines": [3, 8]}
    assert lr["theirs"] == {"file": "f0", "lines": [2, 7]}


def test_runs_never_cross_files():
    a = corpus([1, 2, 3], [4, 5, 6])
    b = corpus([1, 2, 3, 4, 5, 6])
    assert sm.longest_run(a, b)["tokens"] == 3
    assert sm.coverage(a, b, 4)[0] == 0.0
    assert sm.coverage(b, a, 4)[0] == 0.0


def test_longest_run_beyond_the_ladder():
    rng = np.random.default_rng(7)
    x = rng.integers(1, 50, 2100).tolist()
    assert sm.longest_run(corpus(x), corpus([99, *x, 98]))["tokens"] == 2100


def test_empty_and_disjoint():
    assert sm.longest_run(corpus([1, 2]), corpus([3, 4]))["tokens"] == 0
    assert sm.coverage(corpus([1, 2, 3]), sm.Corpus("empty"), 2)[0] == 0.0


def test_floors():
    cell = lambda cov, run: {"coverage_8": cov, "longest_run": run}  # noqa: E731
    fl = sm.floors([[cell(0.5, 40), cell(0.1, 10)], [cell(0.2, 12), None]])
    assert fl[0]["diagonal"]["coverage_8"] == 0.5
    assert fl[0]["median_coverage_8"] == 0.1 and fl[0]["max_longest_run"] == 10
    assert fl[1]["diagonal"] is None and fl[1]["median_coverage_8"] == 0.2


# --- the file tables -------------------------------------------------------------------------------


def test_afni_upstream_tools_from_the_provenance_table():
    prov = "\n".join(
        [
            "| `tshift` | `src/3dTshift.c` | x | MCW |",
            "| `cli::tshift` | `src/3dTshift.c`, `src/thd_writedset.c` | x | MCW |",
            "| `dataset` | `src/thd_writedset.c` | x | MCW |",
            "| `mask` | `src/thd_automask.c` | x | MCW |",
            "| `unifize` | `src/3dUnifize.c` | x | MCW |",
        ]
    )
    got = sm.afni_upstream_tools(prov)
    assert got == {
        "src/3dTshift.c": "tshift",
        "src/thd_writedset.c": "afni-shared",
        "src/thd_automask.c": "automask",
        "src/3dUnifize.c": "unifize",
    }


@pytest.mark.parametrize(
    ("rel", "table", "tool"),
    [
        ("fsl-bet2/bet2.cpp", sm.FSL_UPSTREAM_TOOLS, "bet"),
        ("fsl-meshclass/mesh.cpp", sm.FSL_UPSTREAM_TOOLS, "bet"),
        ("fsl-flirt/standard_space_roi", sm.FSL_UPSTREAM_TOOLS, "standard_space_roi"),
        ("fsl-flirt/flirt.cc", sm.FSL_UPSTREAM_TOOLS, "flirt"),
        ("fsl-avwutils/fslroi.cc", sm.FSL_UPSTREAM_TOOLS, "fslroi"),
        ("fsl-avwutils/fslstats.cc", sm.FSL_UPSTREAM_TOOLS, "fslmaths"),
        ("fsl-newimage/costfns.cc", sm.FSL_UPSTREAM_TOOLS, "fsl-shared"),
        ("larmorx-nc-flirt/src/ssroi.rs", sm.FSL_REPLICA_TOOLS, "standard_space_roi"),
        ("larmorx-nc-core/src/basisfield/splinefield.rs", sm.FSL_REPLICA_TOOLS, "topup"),
        ("larmorx-nc-avwutils/src/fslroi.rs", sm.FSL_REPLICA_TOOLS, "fslroi"),
        ("larmorx-nc-core/src/volume.rs", sm.FSL_REPLICA_TOOLS, "fsl-shared"),
    ],
)
def test_fsl_tables(rel, table, tool):
    assert sm._fsl_tool(rel, table) == tool


def test_afni_modules():
    assert sm._afni_module("crates/larmorx-afni-replica/src/cli/tshift.rs") == "cli::tshift"
    assert sm._afni_module("crates/larmorx-afni-replica/src/cli/mod.rs") == "cli"
    assert sm._afni_module("crates/larmorx-afni-replica/src/csfft.rs") == "csfft"


# --- a whole run on a synthetic plan ---------------------------------------------------------------


def test_run_on_a_synthetic_plan(tmp_path, monkeypatch):
    files = {
        "up/total.c": C_SNIPPET,
        "up/other.c": "int g(int q) { while (q > 3) q = q / 2 - helper(q); return q; }\n",
        "rep/total.rs": RUST_SNIPPET,
        "rep/other.rs": "fn g(mut q: i32) -> i32 { while q > 3 { q = q / 2 - helper(q); } q }\n",
        "cr/total.rs": INDEPENDENT_RUST,
        "cr/other.rs": "pub fn halve_until_small(v: i32) -> i32 { let mut k = v; k }\n",
    }
    for rel, text in files.items():
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / rel).write_text(text, encoding="utf-8")
    plan = sm.Plan()
    for tool in ("total", "other"):
        for role, d, ext in (
            ("cleanroom", "cr", "rs"),
            ("upstream", "up", "c"),
            ("replica", "rep", "rs"),
        ):
            p = tmp_path / d / f"{tool}.{ext}"
            plan.add("FSL", tool, role, sm.Source(str(p), f"{d}/{tool}.{ext}"))
    plan.calibration.append(
        (
            "a translation",
            "port",
            [plan.tools["FSL"]["total"]["replica"][0]],
            [plan.tools["FSL"]["total"]["upstream"][0]],
        )
    )
    monkeypatch.setattr(sm, "make_plan", lambda *a: plan)
    res = sm.run(tmp_path, tmp_path, None, False, 1)
    total = res["tools"]["FSL/total"]
    n = sm.NS["names"][0]
    rep_cov = total["replica_vs_upstream"]["names"][f"coverage_{n}"]
    cr_cov = total["cleanroom_vs_upstream"]["names"][f"coverage_{n}"]
    assert rep_cov > 0.5 and cr_cov == 0.0
    assert total["replica_vs_upstream"]["names"]["longest_run"]["tokens"] > 20
    assert res["calibration"][0]["names"][f"coverage_{n}"] == rep_cov
    assert res["matrices"]["FSL"]["tools"] == ["total", "other"]
    diag = res["matrices"]["FSL"]["replica_vs_upstream_floors"][0]["diagonal"]
    assert diag["longest_run"] > 20
    out = tmp_path / "similarity.md"
    sm.write_markdown(res, out, "abc1234")
    text = out.read_text(encoding="utf-8")
    assert "## Calibration pairs" in text and str(tmp_path) not in text
