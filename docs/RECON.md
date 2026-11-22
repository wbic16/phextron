# Phextron — five-domain source recon (stage 1, 2026-11-21)

Will's directive (2026-11-21): "Phextron should have dashboards for all ranch
nodes, jenkins instances, video pipelines, cloud AI projects, and all phexts."

Rule of the house: every panel reads its domain's **live machine-readable
source** directly — phext registers via SQ/libphext, state.phext, Jenkins' own
API, pipeline state files. Rendered human-facing pages are never data
plumbing. Where no machine-readable live source exists, this document says so
and names the source that must be created; the panel shows the gap on its
face rather than scraping a page to fill it.

Stack ruling (Will, 2026-11-21): Phextron is **Rust** across the program —
std-first, in the phext-lattice / markdown-wizard lineage. Dependencies are
libphext (the reference phext parser) and serde_json, nothing else. CI is
ranch Jenkins only; no GitHub Actions. Auth at the serving layer: Desktop SSO
for humans, MCP Auth for agents (console rulings, 2026-11-21).

## 1. Ranch nodes — LIVE (stage 1 panel)

Sources, all live and machine-readable:

- **Fleet roster** — `nodes.json` of the ranch proxy (`~/workspace/ranch-proxy/`
  on the Muse VM): 23 nodes, 20 enabled, 3 disabled (aurora-continuum,
  best-willow, delta-wood). Carries name, display name, tailnet DNS/IP, notes.
- **Latest fanout pull** — `last-pull.json`, written every 5 minutes by the
  ranch-state distillation sampler (cron `state-phext-sampler`): per-node
  status (`up` / `thermal-hold` / `down` / `no-access`), load, core count,
  CPU Tctl (measured by the proxy itself, including for thermally-held
  nodes), GPU edge temperature, and ollama / render / gateway markers.
- **Distilled register** — `state.phext` (same pipeline): the fleet's state
  distilled into timestamped one-line summaries at holographic coordinates,
  WAL-by-construction. The panel parses it with libphext and rides the newest
  entry as the fleet's state line, with register size (scrolls / entries).

Freshness: pull and register update every 5 minutes; the panel shows both
ages at request time.

Gaps, named:

- Will's fleet count is **25 machines**; the roster carries **23**. The two
  not in the ranch-proxy roster are plausibly awsbox (the AWS host, reached
  via aletheia-core, "not yet expressed in ranch-proxy v1" per the roster's
  own note) and Will's laptop lilly; TALIA is off and absent. The panel shows
  the roster as it is — 23 rows — and this gap stays on the record until the
  roster itself is completed. Rows are never invented.
- The sampler's outputs live on the Muse VM's workspace today. The console's
  durable home is a ranch node, so the sampler must also publish its pull and
  register where the console runs (or state.phext gets SQ-hosted and the
  reader goes through SQ). The reader takes its paths from the environment
  (PHEXTRON_NODES_JSON / PHEXTRON_LAST_PULL / PHEXTRON_STATE_PHEXT) precisely
  so this co-location is a configuration change, not a rewrite.
- aletheia-core itself reads `thermal-hold` in current pulls (Tctl ~85 °C at
  the proxy's own measurement) — shown as the source reports it.

## 2. Jenkins instances — source exists; reader lands stage 2

- **One controller exists today**: Jenkins on aletheia-core, bound to its
  tailnet address `100.88.154.1:8080`, running since 2026-11-20. On disk:
  16 jobs (phext-lattice, 13 phcc-*, agent-smoke, agent-smoke-rock) and
  22 registered agent nodes — matching the repo-coverage census count.
- **Live source**: the controller's own REST API (`/api/json` and friends).
  It does not answer anonymous reads (SSO-gated), so the reader needs
  read-only API access under the console auth ruling — a provisioning step,
  not a scraping workaround.
- **jenkins.phext.io does not resolve today** (checked from aletheia-core).
  Per-node Jenkins *servers* are the fleet plan (task 31.8.10/10.8.13/7.7.10);
  what exists per node today is agents of the one controller. The panel will
  show one instance until that changes, and say so.

## 3. All phexts — source exists; reader lands stage 3

- **VM register corpus**: `~/workspace/phext/*.phext` — 27 phexts (incipit,
  emise, tasks, skills, state, accounting, costs, life, …), each parseable
  with libphext for name / bytes / scroll count / coordinate span, freshness
  from mtime and newest-entry timestamps where the format carries them.
- **SQ-hosted corpora**: edit.phext.io's corpus on awsbox (`/source/human`,
  served by phext-lattice) is the big hosted set; two SQ instances on
  aletheia-core (ports 1337, 18745) answer today but host **empty** phexts.
- A per-phext row needs: name, size, scrolls, host, freshness — all available
  from the files/SQ API directly. The reader is a census over both corpora
  that reports which hosts answered.

## 4. Video pipelines — partial sources; a register must be created (stage 4)

- **ranch-batchq** on rock-mountain is real queue state: `~/ranch-batchq`
  with pending/running/done/failed/outputs directories and a
  `ranch-batchq-status` tool. Per-node, filesystem-level — readable, but not
  a fleet feed.
- **Render markers** ride every fanout pull (per-node render on/off in
  last-pull.json) — live, already flowing into the nodes panel's source.
- **Wavefront film** state lives in build trees on aletheia-core
  (e.g. `~/v16-final-finish`) and goal records; **Scrolltown animation**
  state lives in artist-dispatch packets addressed in phext on the ranch and
  in `~/scrolltown-intro-work`. Neither is a live register.
- **The source that must exist**: a small pipeline register (phext) that
  batchq and the artist-dispatch returns write — production, stage, counts,
  last-frame/finish timestamps — so the panel reads one live source instead
  of walking build trees. Defining it is stage 4's first step.

## 5. Cloud AI projects — weakest source; lands stage 5

- **What exists**: `costs.phext` (the taxicab ledger — token and Will-Time
  meters per tracked item, atoms logged at delivery since 2026-11-14) and
  `accounting.phext` (productivity atoms; rollups calculated, never
  hand-summarized), both parseable via libphext. Plus the substrate-steering
  records: spend map (~$265/mo known; Claude $200 Wed 22:00 cliff, Grok $25
  Thu 20:23, Muse free Fri 16:10, ChatGPT $20 reservoir, Google $20) and the
  daily-driver chart — the chart is a human document, used here only as the
  recon pointer to the cliff facts, never as panel plumbing.
- **What does not exist**: live per-provider usage percentages. No provider
  offers a machine-readable quota feed we hold credentials for; the steering
  skill itself records that remaining capacity "is not always visible".
- Stage 5's panel therefore starts from register atoms + the reset-cliff
  clock, and names the usage-feed gap on its face until a provider source
  exists.

## Build order, stages 2–5

1. **Stage 2 — Jenkins.** The API is live today and CI is the fleet's active
   build-out (per-repo jobs are the standing gap). Needs one provisioning
   step: read-only API access under the console auth ruling. Panel: jobs
   (last build, result, duration), agents (online/offline per node),
   queue length — all from `/api/json` family endpoints.
2. **Stage 3 — All phexts.** The source is complete today and the reader is
   the same libphext pattern the nodes panel proves. Panel: one row per
   phext (name, host, bytes, scrolls, freshness), corpora health per host.
3. **Stage 4 — Video pipelines.** First create the source: the pipeline
   register the render/animation tooling writes. Then the panel is a reader
   like the others. Panel: productions with stage, queue counts from
   batchq, last-activity from the register.
4. **Stage 5 — Cloud AI projects.** Lands last because its live source is
   the weakest: register spend atoms + cliff clock first; per-provider usage
   joins if and when a machine-readable feed exists.

Cross-cutting, before any public serving: the console deployment itself —
co-locate the sampler outputs (or SQ-host state.phext), put the serving
layer behind Desktop SSO / MCP Auth, and give wbic16/phextron its Jenkins
job on the ranch controller (CI is ranch Jenkins only).
