# SPDX-License-Identifier: Apache-2.0
"""Instruments a scratch copy of the larmorx tree (never a commit): with
``LARMORX_NC_DEBUG=<volume>:<scale>`` the replica's mcflirt then prints every cost evaluation
of that volume at that scale, the affine's 16 doubles (column-major) and the cost as bits, in
the line format of ``libcost_trace.so`` (``DBG <volume> <scale> <hex,...> <cost hex> <cost>``).

Usage: ``python -P debug_patch.py <copy of the larmorx tree>``, then build ``crates-nc`` there.
"""

import sys
from pathlib import Path

p = Path(sys.argv[1]) / "crates-nc/larmorx-nc-mcflirt/src/run.rs"
s = p.read_text(encoding="utf-8")
s = s.replace(
    """        let c = cf.cost(&affmat);
""",
    """        let c = cf.cost(&affmat);
        DBG.with(|d| {
            if let Some((v, sc)) = *d.borrow() {
                if std::env::var("LARMORX_NC_DEBUG").ok().as_deref() == Some(&format!("{v}:{sc}")) {
                    let hex: Vec<String> =
                        (0..16).map(|k| format!("{:016x}", affmat.get(k).to_bits())).collect();
                    eprintln!("DBG {v} {sc} {} {:08x} {:e}", hex.join(","), c.to_bits(), c);
                }
            }
        });
""",
    1,
)
s = s.replace(
    """    let search = Search {
        o,
        st,
        corrupted_at: std::cell::Cell::new(None),
    };""",
    """    DBG.with(|d| *d.borrow_mut() = Some((i, scaling)));
    let search = Search {
        o,
        st,
        corrupted_at: std::cell::Cell::new(None),
    };""",
    1,
)
s += """
thread_local! {
    static DBG: std::cell::RefCell<Option<(i32, f32)>> = const { std::cell::RefCell::new(None) };
}
"""
p.write_text(s, encoding="utf-8")
print("patched", p)
