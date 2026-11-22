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

**Stage 2 outcome (2026-11-21):** the reader is built (`src/jenkins.rs` in
phextron-dash) — jobs with last-build result/duration/age, the queue, and
the 22 agents with online/offline state, all from the REST API, read by a
small std HTTP client (no new dependencies). Two facts changed since the
recon above:

- **The controller's realm is now the house LDAP** (`LDAPSecurityRealm`,
  verified in the live `config.xml`), not the local user database the
  stage-1 setup created. The staged local-admin credential file
  (`/var/lib/jenkins/admin-credentials.txt`) no longer authenticates —
  basic auth with it answers 401 — and anonymous reads answer 403. House
  scripts that still read that file for API checks will fail the same way;
  that is a gap in the source's own housekeeping, recorded here.
- Because the realm is LDAP, the reader's read-only identity must be
  provisioned — a step only Will can take. Until then the panel degrades
  honestly: it asks the API, gets the 403, and says so on its face. The
  reader was verified end to end against a scratch Jenkins 2.580.2 (the
  production controller's exact version, stood up on loopback on
  aletheia-core, production untouched) whose API responses are the test
  fixtures in `crates/phextron-dash/tests/fixtures/`.

Provisioning steps (Will's hands, one time):

1. Create a read-only service account for the console in the house LDAP
   (suggested name: `phextron-ro`) with a strong password
2. Sign in to Jenkins as an administrator
3. Open Manage Jenkins → Security → Authorization (project matrix)
4. Add `phextron-ro` to the matrix
5. Grant it Read only: Overall/Read, Job/Read, Agent/Read — no Build,
   Configure, or Administer grants
6. Sign in to Jenkins as `phextron-ro`
7. Open the account menu → Configure → API Token → Add new Token
8. Name the token `phextron-dash` and click Generate
9. Set `PHEXTRON_JENKINS_USER=phextron-ro` and
   `PHEXTRON_JENKINS_TOKEN=<the generated token>` in the console process's
   environment (never in the repo)
10. Restart the console — the Jenkins panel goes live and shows
    "reading as phextron-ro" on its face

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

**Stage 3 outcome (2026-11-21):** the reader is built (`src/phexts.rs` in
phextron-dash) — a census over both corpora where every row is computed
from the file itself with libphext: bytes from the filesystem, scroll
count from the span index (`spans`, whose documented contract is exactly
`explode`'s — a scroll exists iff it holds at least one byte; a unit test
pins the two parsers to the same count), freshness from mtime. A phext
that does not parse keeps its row with the failure named; its scroll
count is never guessed. The census is cached per corpus behind a
stat-only fingerprint (name, length, mtime at nanosecond precision), so
a request re-stats and re-parses only what changed — ietf.phext alone is
564,589,207 bytes / 10,067 scrolls, and a warm panel answer takes
milliseconds. Verified numbers: the VM register corpus reads **27
phexts, 579,750,306 bytes, 19,078 scrolls, 0 parse failures**; the
edit.phext.io corpus reads **49 phexts, 85,169,333 bytes, 390,982
scrolls, 0 parse failures** (its byte total matches an independent
`ls | awk` sum exactly; ultra.phext is its giant at 320,958 scrolls).

Two recon assumptions did not survive contact:

- **The awsbox corpus is not reachable from the ranch nodes today.** The
  hosted files live on awsbox's disk; phext-edit binds 127.0.0.1:8080 on
  awsbox, its only corpus-wide API (/api/phexts) carries names and byte
  sizes but no scroll counts, and the public route is HTTPS-only (the
  port-80 vhost 301-redirects) while this std-only reader speaks plain
  HTTP on the tailnet. The SQ tenant hosts on awsbox (1337/1338/1339/
  8931) answer 401 without a tenant key and host other tenants' phexts,
  not this corpus.
- **A default path for it would have lied.** The first proof run pointed
  the reader at a defaulted `/source/human` and got a clean, plausible,
  wrong answer: aletheia-core itself carries a `/source/human` — a
  wbic16/human checkout from 2026-03-15 (commit b092555), 23 phexts,
  untouched for 251 days — which posed as the live corpus until the rows
  were checked against awsbox. The reader therefore takes the awsbox
  corpus location from `PHEXTRON_PHEXTS_AWSBOX_DIR` **only, with no
  default**: unconfigured, the corpus degrades by name rather than
  guessing. It was verified live on awsbox itself
  (`PHEXTRON_PHEXTS_AWSBOX_DIR=/source/human`), in a real browser, light
  and dark, zero console errors — and the same binary on aletheia-core
  shows the corpus degraded with the reason on its face. The reader goes
  live for awsbox unchanged wherever the files are honestly readable:
  on awsbox, or behind a plain-HTTP tailnet route to the corpus.

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

**Stage 4 outcome (2026-11-21)**: the register now exists and the panel
reads it. `pipelines.phext` lives in `~/workspace/phext` on the house
pattern — `pipelines-source.json` is the source of truth,
`build_pipelines_phext.py` builds it (generated Home scroll, rollups
computed from the entries, `--check` CURRENT by sha256), and
`update_pipeline.py` is the writer path a pipeline updates its own
entry through (edit JSON, stamp, rebuild, verify). One scroll per
pipeline: name, kind (film / animation / training / other), state
summary, last activity, the source its truth lives in, updated stamp.
Seeded with four entries, each verified against its own record at seed
time: Wavefront film program (V16 final delivered 2026-11-09), Scrolltown
animation (active; exercises ex01-ex03 generated and in gate validation
2026-11-21), the phext training video (rough cut 2026-11-10; polish tier
open on Will's steps), and the ranch-batchq standing queue (idle).

The panel (`crates/phextron-dash/src/video.rs`) parses the register with
libphext and reads ranch-batchq two ways: live from the queue
directories where they are readable (`PHEXTRON_VIDEO_BATCHQ_DIR`), else
from the snapshot document the VM-side sync writes beside the other
phextron snapshots (`~/phextron-data/batchq.json`), with the capture
stamp on its face. The two sources degrade independently: register rows
show with the queue section degraded by name, and a missing register
degrades the panel while the freshness line still reports the queue.
One correction to the recon above, from the seed read: the queue's jobs
to date are eval/classify jobs, not renders, and its runner's last
status write was 2026-11-19 (`no_jobs`) — the register entry says both,
and the panel shows the queue as the standing batch queue it is.

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

## STASH — the estate-wide data census (2026-11-21)

Will's directive: "make sure phextron has visibility into everywhere
I've stashed data." The census is a durable phext register Phextron can
read live: **stash.phext** in ~/workspace/phext (source
stash-source.json, builder build_stash_phext.py, --check CURRENT,
sha256 4dcd956e20c41fb13b42aab63b5b49f2829b791e44540c7c1593b54f001ef206;
verified parseable by SQ — toc + select exercised). One scroll per
stash at 1.1.1/1.1.1/1.1.N; the home scroll carries totals computed by
the builder from the entries, never hand-written rollups.

- **67 stashes recorded; 1,953,371,966,239 bytes (~1.95 TB) measured**
  across the 45 counted entries (detail scrolls inside a counted
  parent are marked counted=no so nothing double-counts).
- By kind: working trees 27, other 15, phext corpus 6, film & renders
  4, backups 4, mailboxes 3, registers 2, media 2, site data 2,
  repos 2.
- Coverage today: the VM phext corpus and the awsbox human corpus are
  already read by the phexts panel; the nodes panel sees nodes but not
  their data roots — everything else in the register is new surface.

Surprises the census surfaced:

- **chromium-census, ~240 GB** in the hub's home — the largest single
  stash on aletheia-core, previously in no register.
- The hub's system Ollama store (/usr/share/ollama) is **318 GB**;
  fleet-wide the system stores dwarf the home dirs (lighthouse-omen
  84 GB, ashfall-haven 83 GB, elven-path 78 GB + a 51 GB ~/models).
- **~1.1 TB of the hub's df-used is unattributed** by read-only
  probes (dotdirs, deleted-open files, snapshots, containers and
  >10 GB files all checked and excluded). Recorded in the register as
  the "Hub unattributed remainder" gap; a full du -sb /home/wbic16
  (timed out at 240 s) is the follow-up probe.
- orchid-flow exports **six** NFS shares, not one: shared (134.6 GB —
  lastbook alone is 107 GB), exocortex and mirrorborn (small, incl. a
  phext stash), and models/backups/bliss which are **mounted
  fleet-wide and completely empty**.
- GitHub grew: **242 repos, 5,296,733,184 B** by GraphQL diskUsage
  (2026-11-15's count was 219). Largest: exo-dreams 710,437,888 B.

Unenumerated gaps (named in the register with reasons): Great White
(SSH key refused), aurora-continuum / best-willow / delta-wood
(offline since 2026-11-04), Uncanny Valley (census-measured only;
jared's key grant pending), Neon's container corpus (inside the
Antigravity Podman container, not visible from the WSL2 SSH landing),
the Lilly Windows side (no read path), Google Drive (connector auth
unavailable in this environment), the ni-knight archive root (task
30.4.6/5.5.8/4.4.8 was creating it concurrently — absent at probe
time, re-census when it lands), and three nodes' system Ollama
stores (thermal hold / fanout truncation).

What the Data panel needs: read stash.phext itself (SQ or libphext —
the register is the source, not a scrape of this document); re-probe
on the sampler's cadence via each entry's read_method (filesystem
sampler over tailnet SSH for node/awsbox roots, SQ /api/v2 for hosted
corpora, GitHub GraphQL diskUsage for the account); render unmeasured
entries as named gaps, never as zeroes; and treat counted=no detail
scrolls as drill-down under their counted parent so the estate total
stays honest.

Panel outcome (2026-11-21, task 25.7.9/8.7.10/7.7.10): the Data
panel is live in crates/phextron-dash (src/data.rs). It reads
stash.phext itself with libphext from the synced snapshot
(~/phextron-data/phext-vm/stash.phext; PHEXTRON_DATA_STASH_PHEXT
overrides the location), computes the SHA-256 of the register bytes
it read and puts it on its face (the census build's hash verified
live: 4dcd956e20c41fb13b42aab63b5b49f2829b791e44540c7c1593b54f001
ef206), and renders the Home scroll's builder-computed totals as
written — 67 stashes, 1,953,371,966,239 bytes (1.95 TB) measured
over 45 counted entries, 9 unmeasured gaps. The 13 counted=no
detail scrolls nest under their counted parents (Hub home
(wbic16); GitHub account (wbic16)) and never enter the totals; the
9 unmeasured stashes render as named GAP rows with their reasons.
Where the console host is a stash's own host — declared via
PHEXTRON_DATA_LOCAL_HOSTS, never guessed, since register paths are
host-relative — a stat re-probe refreshes what a stat can honestly
refresh (a file's size and mtime; a directory's mtime only), and a
failed probe falls back to the register value on the row's face;
every other row carries the register value with its census stamp.
A missing or Home-less register degrades by name. Verified in
headless Chromium on aletheia-core: live + degraded, light + dark,
zero console errors (docs/screenshots/data-*.png). Register
observation carried forward: the ni-knight archive root was probed
at 19:32 CST, before the archive (task 30.4.6/5.5.8/4.4.8) landed —
the entry stands as census truth until the next census, and the
panel renders it as written.
