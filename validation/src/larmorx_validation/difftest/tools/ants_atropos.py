# SPDX-License-Identifier: Apache-2.0
"""Atropos: random initializations, likelihood models, posterior formulations, MRF, ICM,
outlier handling and partial-volume classes on head-like phantoms with a mask.

Every case fixes the seed (``-r 0``: ANTs' default seeds from the clock); the parity runner
compares ANTs at one thread, as built and with CORE-MATH preloaded (larmorx bit-identical to the
second), every output and, where asked, standard output.
"""

from __future__ import annotations

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool, group, required
from larmorx_validation.difftest.tools import _ants


def draw(d: gen.Draw) -> dict:
    dim = d.choice("dimension", (3, 2), (0.9, 0.1))
    shape = [d.integer(f"n{k}", 8, 20) for k in range(dim)]
    geometry = gen.plumb_geometry((d.choice("spacing", (1.0, 2.0)),) * 3)
    main = gen.draw_image(
        d.child("a"),
        dim,
        shape=shape,
        geometry=geometry,
        contents=("head", "crop", "blobs"),
        content_weights=(6, 2, 1),
        dtypes=("float32", "int16", "float64"),
        specials=0.0,
        extreme=0.05,
    )
    if main["content"]["type"] == "head":
        main["content"]["size"] = float(min(shape) * geometry["spacing"][0] * 0.45)
        main["content"]["centre"] = [(n - 1) * geometry["spacing"][0] / 2 for n in [*shape, 1][:3]]
        main["content"]["noise"] = d.choice("noise", (1.0, 3.0, 6.0))
    inputs = {"a": main, "mask": _ants.mask_of(main, d.child("mask"), dtype="uint8")}
    k = d.choice("classes", (3, 2, 4), (3, 1, 1))
    init = d.choice(
        "initialization",
        ("KMeans", "KMeans-centres", "Otsu", "Random", "PriorProbabilityImages", "PriorLabelImage"),
        (4, 1, 2, 1, 2, 1),
    )
    options = [f"{init} initialization", f"{k} classes", f"dimension {dim}"]
    if init.startswith("Prior"):
        k = 3
    if init == "KMeans":
        init_text = f"KMeans[{k}]"
    elif init == "KMeans-centres":
        init_text = "KMeans[{},{}]".format(k, "x".join(str(50 * (j + 1)) for j in range(k)))
    elif init in ("Otsu", "Random"):
        init_text = f"{init}[{k}]"
    elif init == "PriorProbabilityImages":
        for j in (1, 2, 3):
            inputs[f"prior{j:02d}"] = {
                **gen.draw_image(
                    d.child(f"prior{j}"),
                    dim,
                    shape=shape,
                    geometry=geometry,
                    contents=("blobs",),
                    dtypes=("float32",),
                    specials=0.0,
                    extreme=0.0,
                    scaling=0.0,
                ),
                "suffix": ".nii.gz",
            }
            inputs[f"prior{j:02d}"]["values"] = {"scale": 0.01, "offset": 0.0}
        weight = d.choice("prior weight", ("0.5", "0.25", "0.8"))
        threshold = d.choice("prior threshold", ("", ",0.1"))
        init_text = f"PriorProbabilityImages[3,{{series:prior}},{weight}{threshold}]"
    else:
        inputs["labels"] = gen.draw_image(
            d.child("labels"),
            dim,
            shape=shape,
            geometry=geometry,
            contents=("labels",),
            dtypes=("uint8",),
            specials=0.0,
            extreme=0.0,
            scaling=0.0,
        )
        inputs["labels"]["content"]["n"] = 3
        inputs["labels"]["suffix"] = ".nii.gz"
        init_text = f"PriorLabelImage[3,{{in:labels}},{d.choice('label weight', ('0.5', '0.25'))}]"
    groups = [
        required("initialization", "-i", init_text),
        required("dimension", "-d", str(dim)),
        required("image", "-a", "{in:a}" if not d.chance("image weight", 0.1) else "[{in:a},0.5]"),
        required("mask", "-x", "{in:mask}"),
        required(
            "convergence",
            "-c",
            d.choice("convergence", ("[3,0]", "[2,0]", "[5,0]", "[3,-1]", "[1,0]")),
        ),
        required("seed", "-r", "0"),
    ]
    if d.chance("second modality", 0.15):
        inputs["b"] = _ants.same_grid(
            main, d.child("b"), contents=("head", "blobs"), specials=0.0, extreme=0.0
        )
        groups.append(group("second modality", "-a", "{in:b}"))
        options.append("two modalities")
    if d.chance("likelihood", 0.35):
        groups.append(
            group(
                "likelihood",
                "-k",
                d.choice(
                    "likelihood model",
                    (
                        "Gaussian",
                        "HistogramParzenWindows[1,32]",
                        "ManifoldParzenWindows[1,10]",
                        "HistogramParzenWindows[0.5,16]",
                    ),
                ),
            )
        )
        options.append("-k")
    if d.chance("mrf", 0.6):
        radius = "x".join(["1"] * dim)
        groups.append(
            group("mrf", "-m", f"[{d.choice('smoothing', ('0.1', '0.3', '0', '0.05'))},{radius}]")
        )
        options.append("-m")
    if d.chance("posterior", 0.35):
        groups.append(
            group(
                "posterior",
                "-p",
                d.choice(
                    "formulation",
                    (
                        "Socrates",
                        "Plato",
                        "Aristotle",
                        "Sigmoid",
                        "Socrates[0]",
                        "Socrates[1,2,0.5,0.1]",
                        "Socrates[1]",
                    ),
                ),
            )
        )
        options.append("-p")
    if d.chance("icm", 0.25):
        groups.append(group("icm", "-g", d.choice("icm", ("0", "1"))))
        options.append("-g")
    if init.startswith("Prior") and d.chance("euclidean", 0.3):
        groups.append(group("euclidean", "-e", d.choice("euclidean value", ("0", "1"))))
        options.append("-e")
    if init.startswith("Prior") and d.chance("label propagation", 0.2):
        groups.append(group("label propagation", "-l", "[0.5,0.8]"))
        options.append("-l")
    if d.chance("outliers", 0.15):
        groups.append(
            group(
                "outliers",
                "-w",
                d.choice("outlier handling", ("BoxPlot[0.25,0.75,1.5]", "GrubbsRosner[0.05,0.1]")),
            )
        )
        options.append("-w")
    if k == 3 and d.chance("partial volume", 0.1):
        groups.append(group("partial volume", "-s", "1x2"))
        options.append("-s")
    groups.append(required("output", "-o", "[{out}/seg.nii.gz,{out}/post%02d.nii.gz]"))
    outputs = ["seg.nii.gz"] + [f"post{j:02d}.nii.gz" for j in range(1, k + 1)]
    if "-s" in options:
        outputs.append(f"post{k + 1:02d}.nii.gz")
    return {
        "groups": groups,
        "inputs": inputs,
        "options": options,
        "extra": {
            "outputs": outputs,
            "min_size": 6,
            "keep_inputs": ["prior01", "prior02", "prior03"],
        },
    }


def case(
    recipe: dict,
    impl: str,
    *,
    case_prefix: str | None = None,
    cid: str | None = None,
    description: str | None = None,
    **kw,
):
    from larmorx_validation.difftest import inputs
    from larmorx_validation.parity.ants_atropos import AtroposCase
    from larmorx_validation.parity.harness import Case

    prefix = _ants.prefix_of(recipe, case_prefix)
    args = list(_ants.gen_args(recipe, prefix))
    args = [
        a.replace("{series:prior}", "{series:dt:" + inputs.key(prefix, "prior") + "}") for a in args
    ]
    c = AtroposCase(
        cid or _ants.case_id(recipe),
        "difftest",
        description or _ants.describe(recipe),
        tuple(args),
        outputs=tuple(recipe["extra"]["outputs"]),
        **kw,
    )
    return Case(c.id, c.category, c.description, c)


def run(impl: str):
    from larmorx_validation.parity.ants_atropos import run_case

    return run_case


TOOL = Tool(
    name="ants-atropos",
    title="Atropos",
    suite="ants-atropos",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    timeout=300,
    weight=1.2,
    unavailable=_ants.ants_unavailable,
    crash_kw={"expect": "abort"},
    payload_fields=("tier", "expect", "reason", "silent_ok"),
)
