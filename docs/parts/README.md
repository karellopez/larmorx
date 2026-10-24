# The parts: briefs, reports, handoffs

Every part of larmorx's work was done by an agent from a written brief, and ended with the
agent's final report and, since 2026-10-23, a handoff of the text for the shared documents. They
are kept here verbatim, one directory per part (`<date>-<part>/brief.md`, `report.md`,
`handoff.md`), so how each tool was specified, what its agent found and what it reported stays
with the code. Paths in them are the development machine's; the decisions they led to are in
[decision-log.md](../decision-log.md), the state of each tool in its validation record. The
orchestrator adds each new part at its merge (`scripts/orchestration/README.md`).

**Restricted for clean-room implementers.** Many parts were done by replica authors and spec writers who
read the upstream source; their briefs, reports and handoffs contain source-derived text (some handoffs have
sections marked †). **A clean-room implementer must not read `docs/parts/`** (CLAUDE.md rule 1).
