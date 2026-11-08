# SPDX-License-Identifier: Apache-2.0
import sys

import bokeh
import joblib
import mapca
import matplotlib
import nibabel
import nilearn
import numpy
import pandas
import robustica
import scipy
import scipy.linalg
import scipy.optimize
import sklearn
import tedana
import threadpoolctl
import tqdm
from threadpoolctl import threadpool_info

print(
    sys.version.split()[0],
    "numpy",
    numpy.__version__,
    "scipy",
    scipy.__version__,
    "nibabel",
    nibabel.__version__,
    "nilearn",
    nilearn.__version__,
    "pandas",
    pandas.__version__,
    "joblib",
    joblib.__version__,
    "tqdm",
    tqdm.__version__,
    "tpc",
    threadpoolctl.__version__,
    "tedana",
    tedana.__version__,
    "mapca",
    mapca.__version__,
    "robustica",
    robustica.__version__,
    "bokeh",
    bokeh.__version__,
    "mpl",
    matplotlib.__version__,
    "sklearn",
    sklearn.__version__,
)
for i in threadpool_info():
    print(" ", i["filepath"].split("/")[-1], i["version"], i.get("architecture"), i["internal_api"])
