# phextron-dash — the console dashboards

Will's directive (2026-11-21): *"Phextron should have dashboards for all
ranch nodes, jenkins instances, video pipelines, cloud AI projects, and all
phexts."*

One shell, one panel per domain, each panel fed by its own **live-source
reader** — phext registers via SQ/libphext, state.phext, Jenkins' own API,
pipeline state files. Rendered human-facing pages are never used as data
plumbing. A domain whose live source does not exist yet gets a panel that
names the gap on its face; the five-domain source map and the stage 2–5
build order live in `../../docs/RECON.md`.

Stage 1: the framework plus the **ranch-nodes panel** live end to end —
fleet roster (ranch-proxy `nodes.json`), the sampler's latest fanout pull
(`last-pull.json`: status, load, Tctl, GPU, service markers), and the
distilled `state.phext` register parsed with libphext.

Stage 2: the **Jenkins panel** live end to end — the fleet's one controller
(aletheia-core) read through its own REST API: jobs with last-build result,
duration and age, the build queue, and the controller's registered agents
(the ranch nodes) with online/offline state. The controller's realm is the
house LDAP and anonymous reads are refused, so the reader authenticates
with a read-only API token from the environment (`PHEXTRON_JENKINS_URL`,
`PHEXTRON_JENKINS_USER`, `PHEXTRON_JENKINS_TOKEN`). Until that identity is
provisioned (steps in `../../docs/RECON.md` §2) the panel degrades
honestly, naming the refusal it got; it never fabricates job or agent
state.

Stage 3: the **All Phexts panel** live end to end — a census over the two
phext corpora, every row computed from the file itself with libphext
(bytes, scroll count, mtime): the register corpus from the Muse VM
(`~/workspace/phext`, read via `PHEXTRON_PHEXTS_VM_DIR`) and the
edit.phext.io corpus on awsbox (`/source/human`, read via
`PHEXTRON_PHEXTS_AWSBOX_DIR`). A phext that does not parse keeps its row
and says so; its scroll count is never guessed. From the ranch nodes the
awsbox corpus degrades with its reason on its face — the hosted files
live on awsbox's disk, phext-edit binds localhost there, and its
corpus-wide API carries no scroll counts — and the same reader goes live
unchanged where the files are readable. The awsbox corpus deliberately
has no default path — it is read only where `PHEXTRON_PHEXTS_AWSBOX_DIR`
is set — because a stale namesake `/source/human` sat untouched on
aletheia-core for 251 days, and a guessed default would have dressed
that checkout up as the live corpus (docs/RECON.md §3).

Stack: Rust across the program (Will's ruling, 2026-11-21) — std-first HTTP
in the phext-lattice / markdown-wizard lineage. This crate's dependencies
are `libphext` (the reference phext parser) and `serde_json`, nothing else.
Dark mode follows the visitor's OS preference, with a stored
System/Light/Dark override.

Serving-layer auth follows the console rulings: Desktop SSO for humans, MCP
Auth for agents. Stage 1 binds localhost and carries no auth of its own;
the gate lands with the console deployment.

## Run

```
cargo build --release -p phextron-dash
PHEXTRON_NODES_JSON=/path/to/nodes.json \
PHEXTRON_LAST_PULL=/path/to/last-pull.json \
PHEXTRON_STATE_PHEXT=/path/to/state.phext \
PHEXTRON_JENKINS_URL=http://100.88.154.1:8080 \
PHEXTRON_JENKINS_USER=<read-only identity> \
PHEXTRON_JENKINS_TOKEN=<that identity's API token> \
PHEXTRON_PHEXTS_VM_DIR=/home/wbic16/phextron-data/phext-vm \
PHEXTRON_PHEXTS_AWSBOX_DIR=/source/human \
PHEXTRON_PORT=8901 ../../target/release/phextron
```

Then open http://127.0.0.1:8901/ — JSON at `/api/panels` and
`/api/panels/<id>` (nodes, jenkins, video, cloud, phexts).

## Adding a panel

Implement the `Panel` trait (`src/panels.rs`), register it, done — the
shell, routing, and theme are already carried. A reader that cannot read
its source reports `degraded` with the reason; it never fabricates data.
