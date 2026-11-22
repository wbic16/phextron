//! Recon panels for the four domains whose readers land in stages 2–5.
//! Each names its live source (from docs/RECON.md), where that source runs,
//! how fresh it is, and what the reader still needs. No fabricated data:
//! a domain whose reader is not built yet says so on its face.

use crate::panels::{Panel, PanelData, PanelState, SourceInfo};
use serde_json::json;

pub struct PendingPanel {
    id: &'static str,
    title: &'static str,
    stage: u8,
    source_name: String,
    source_location: String,
    freshness: String,
    needs: String,
}

impl PendingPanel {
    pub fn jenkins() -> Self {
        PendingPanel {
            id: "jenkins",
            title: "Jenkins Instances",
            stage: 2,
            source_name: "Jenkins controller REST API (/api/json)".into(),
            source_location: "aletheia-core — http://100.88.154.1:8080 (tailnet); 16 jobs, 22 agents registered today".into(),
            freshness: "live API, answers in real time once the reader holds read access".into(),
            needs: "Read-only API access under the console auth ruling (Desktop SSO for humans, MCP Auth for agents). jenkins.phext.io does not resolve today; per-node Jenkins servers are planned (fleet task) but only agents exist so far — the panel will show the one controller honestly until that changes.".into(),
        }
    }

    pub fn video() -> Self {
        PendingPanel {
            id: "video",
            title: "Video Pipelines",
            stage: 4,
            source_name: "ranch-batchq queue state + render markers in the ranch fanout pull".into(),
            source_location: "rock-mountain ~/ranch-batchq (pending/running/done/failed dirs + ranch-batchq-status); Wavefront and Scrolltown build state on aletheia-core".into(),
            freshness: "queue dirs are live filesystem state; render markers refresh with every 5-minute fanout pull".into(),
            needs: "A fleet-wide pipeline register does not exist yet: queue state is per-node directories and film/animation progress lives in goal records and build trees. Stage 4 first defines the small phext register the pipelines write, then reads it.".into(),
        }
    }

    pub fn cloud() -> Self {
        PendingPanel {
            id: "cloud",
            title: "Cloud AI Projects",
            stage: 5,
            source_name: "costs.phext + accounting.phext registers; substrate reset cliffs".into(),
            source_location: "~/workspace/phext registers (via libphext); cliffs from the substrate-steering records (Claude Wed 22:00, Grok Thu 20:23, Muse Fri 16:10 America/Chicago)".into(),
            freshness: "registers update as work is delivered; cliffs are fixed weekly points".into(),
            needs: "Live per-provider usage percentages are not machine-readable anywhere today — no provider feed exists to read. The stage-5 panel starts from what does exist: register spend atoms (calculated, never summarized by hand) plus the reset-cliff clock.".into(),
        }
    }

    pub fn phexts() -> Self {
        PendingPanel {
            id: "phexts",
            title: "All Phexts",
            stage: 3,
            source_name: "the phext corpus itself, parsed with libphext; SQ-hosted corpora via the SQ REST API".into(),
            source_location: "~/workspace/phext (27 registers on the Muse VM) + edit.phext.io corpus on awsbox (/source/human) + SQ instances on aletheia-core".into(),
            freshness: "file mtimes + SQ status; registers change as their builders run".into(),
            needs: "A census pass over every corpus: name, bytes, scroll count, host, freshness per phext. The aletheia-core SQ instances host empty phexts today, so the live corpora are the VM register set and awsbox — the reader enumerates both and says which hosts answered.".into(),
        }
    }
}

impl Panel for PendingPanel {
    fn id(&self) -> &'static str {
        self.id
    }
    fn title(&self) -> &'static str {
        self.title
    }
    fn data(&self) -> PanelData {
        PanelData {
            id: self.id,
            title: self.title,
            state: PanelState::Recon,
            source: SourceInfo {
                name: self.source_name.clone(),
                location: self.source_location.clone(),
                freshness: self.freshness.clone(),
            },
            body: json!({
                "stage": self.stage,
                "needs": self.needs,
            }),
        }
    }
}
