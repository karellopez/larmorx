# Investigations

Tools behind findings: each directory reproduces how one finding about an original program was
established (a register entry in `docs/findings/upstream-defects.md`), with a README saying what it
shows and how to run it. Probes that compare larmorx with an instrumented copy of an original are in
[../probes/](../probes/). Rule (2026-10-23): a tool that produced a finding or a validation record is
committed here or there, never left in a temporary folder.
