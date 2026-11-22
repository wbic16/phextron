# Phextron — Tech Debt Register

Will's ruling (2026-11-21): tech debt is allowed **this one time**, to
improve velocity. The condition is that it is written down here, at the
time it is taken on. We come back and fix it all later.

Rules of this register:

- **No silent debt.** An overly complex subsystem that is not recorded
  here is not sanctioned debt — it is a defect.
- Every entry names: the subsystem, what the complexity is and where it
  lives, why it was allowed, what it costs us, and the proper fix owed.
- An entry leaves this register only when its proper fix has landed —
  never when the fix is merely scheduled.
- This is a one-time allowance for the current push, not a standing
  change to the proper-fixes rule. New debt taken on after this push
  needs Will's word again.

---

## TD-001 — VM-resident state samplers

- **Subsystem:** Phextron dashboard state sampling (the samplers behind
  the panels' `last-pull.json` / state snapshots).
- **What / where:** the samplers and their outputs live on Emise's Muse
  VM. That VM is replaced roughly every two hours, so each replacement
  gaps the panels' sources until the next sync cycle re-syncs them.
  Flagged in the stage-1 follow-up, 2026-11-21.
- **Why allowed:** velocity — the samplers were stood up where the work
  already was, to get the panels live the same evening.
- **Cost:** panels periodically degrade or show stale state through no
  fault of their sources; freshness guarantees are soft until this is
  fixed.
- **Proper fix owed:** run the samplers on a ranch node and keep
  Phextron's state on-node (SQ / registers), so VM replacement is not
  in the panels' supply chain at all.
- **Status:** open (recorded 2026-11-21; verify live state when the
  fix pass runs).
