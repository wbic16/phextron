//! Recon panel for the one domain whose reader lands in stage 5.
//! (Jenkins graduated to a live panel in stage 2 — see `jenkins.rs`;
//! phexts graduated in stage 3 — see `phexts.rs`; video graduated in
//! stage 4 — see `video.rs`.)
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
