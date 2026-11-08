import sys, os, tempfile, numpy as np, nibabel as nb
from pathlib import Path

sys.path.insert(0, os.path.dirname(sys.argv[1]))
import measure_sdc_g as m
import larmorx as lx
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_versions

rng = np.random.default_rng(7)
u = m.phantom(rng)
f = m.true_field()
shift = f * m.RO
aff = m.affine_of("LAS")
blips = [m.distorted(u, shift, 1, +1.0), m.distorted(u, shift, 1, -1.0)]
pair = np.stack(blips, -1).astype(np.float32)
for codes in ("s2q0", "s1q1"):
    with tempfile.TemporaryDirectory() as t:
        t = Path(t)
        img = nb.Nifti1Image(pair, aff)
        if codes == "s1q1":
            img.header.set_qform(aff, 1)
            img.header.set_sform(aff, 1)
        img.to_filename(t / "pair.nii.gz")
        np.savetxt(
            t / "acq.txt", np.array([[0, 1, 0, m.RO], [0, -1, 0, m.RO]]), fmt="%d %d %d %.8f"
        )
        env = {
            "PATH": "/usr/bin:/bin",
            "FSLDIR": str(fsl_versions.fsldir()),
            "HOME": str(t),
            "FSLOUTPUTTYPE": "NIFTI_GZ",
            **fsl_versions.openblas_env(),
        }
        r = oracle_cache.run(
            [
                str(fsl_versions.program("topup")),
                "--imain=pair",
                "--datain=acq.txt",
                f"--config={m.SDCFLOWS_DATA / 'b02b0.cnf'}",
                "--out=tu",
                "--fout=fo",
            ],
            env=env,
            cwd=t,
            outputs=[t],
            inputs=[t / "pair.nii.gz", t / "acq.txt"],
            policy=oracle_cache.Policy(threads=1, family="fsl"),
            runner="sdcflows-sdc07",
            timeout=1800,
        )
        fsl_coef = nb.load(t / "tu_fieldcoef.nii.gz").get_fdata()
        tu = lx.fsl.topup(
            t / "pair.nii.gz",
            np.array([[0, 1, 0, m.RO], [0, -1, 0, m.RO]]),
            config=m.SDCFLOWS_DATA / "b02b0.cnf",
            n_threads=8,
        )
        lc = np.asarray(tu.coefficients.data, dtype=float)
        print(
            codes,
            "rc",
            r.returncode,
            "FSL coef vs clean-room r=%.4f mirrored r=%.4f"
            % (
                np.corrcoef(fsl_coef.ravel(), lc.ravel())[0, 1],
                np.corrcoef(fsl_coef[::-1].ravel(), lc.ravel())[0, 1],
            ),
        )
