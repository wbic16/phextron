//! The panel contract. One panel per dashboard domain; every panel is fed by
//! its own live-source reader and nothing else. Adding a domain is a fill-in:
//! implement `Panel`, register it in `registry()`, and the shell picks it up.
//!
//! A panel is in exactly one of two states:
//! - `live`  — its reader reads the domain's live machine-readable source.
//! - `recon` — the source has been identified (see docs/RECON.md) but the
//!   reader is not built yet. A recon panel names its source and what the
//!   reader still needs. It never fabricates data to fill the space.

use serde_json::Value;

#[derive(Clone, Copy, PartialEq)]
pub enum PanelState {
    Live,
    Recon,
    Degraded,
}

impl PanelState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PanelState::Live => "live",
            PanelState::Recon => "recon",
            PanelState::Degraded => "degraded",
        }
    }
}

/// Where a panel's data comes from, stated on the panel itself.
/// `freshness` is a human line computed by the reader at request time
/// (e.g. "pull file updated 42s ago"), never a copied timestamp alone.
pub struct SourceInfo {
    pub name: String,
    pub location: String,
    pub freshness: String,
}

pub struct PanelData {
    pub id: &'static str,
    pub title: &'static str,
    pub state: PanelState,
    pub source: SourceInfo,
    pub body: Value,
}

impl PanelData {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "title": self.title,
            "state": self.state.as_str(),
            "source": {
                "name": self.source.name,
                "location": self.source.location,
                "freshness": self.source.freshness,
            },
            "body": self.body,
        })
    }
}

pub trait Panel: Send + Sync {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn data(&self) -> PanelData;
}

/// The five domains of Will's 2026-11-21 directive, in shell order,
/// plus the Data panel (stash task 25.7.9/8.7.10/7.7.10): Phextron's
/// visibility into everywhere Will has stashed data, read from the
/// stash.phext census register — the Substrates panel (W4 plan
/// §1.10, task 31.8.11/9.7.13/8.7.10): per-substrate dispatched-work
/// and budget-check coverage against the reset-window clock — and
/// the Inkling APIs panel (task 20.7.10/10.7.12/7.8.8): every
/// Inkling endpoint on the ranch, from the wiring.phext roster and
/// the inkling-apis.phext deployment register.
pub fn registry() -> Vec<Box<dyn Panel>> {
    vec![
        Box::new(crate::nodes::NodesPanel),
        Box::new(crate::jenkins::JenkinsPanel),
        Box::new(crate::video::VideoPanel),
        Box::new(crate::pending::PendingPanel::cloud()),
        Box::new(crate::phexts::PhextsPanel),
        Box::new(crate::data::DataPanel),
        Box::new(crate::substrates::SubstratesPanel),
        Box::new(crate::inkling::InklingPanel),
    ]
}
