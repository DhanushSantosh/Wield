//! Capability and per-tool availability reporting.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wield_core::{AvailabilityView, Requires};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAvailability {
    pub id: String,
    pub title: String,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesReport {
    pub binaries: BTreeMap<String, bool>,
    pub portals: BTreeMap<String, u32>,
    pub tools: Vec<ToolAvailability>,
}

/// Evaluate one descriptor requirement against startup capability data.
pub fn is_available(
    availability: &AvailabilityView,
    requires: &Requires,
) -> (bool, Option<String>) {
    match requires {
        Requires::None => (true, None),
        Requires::Binary(binary) if availability.binaries.contains(binary) => (true, None),
        Requires::Binary(binary) => (false, Some(format!("{binary} is not installed"))),
        Requires::Portal { iface, min_ver } => match availability.portals.get(iface) {
            Some(version) if version >= min_ver => (true, None),
            Some(version) => (
                false,
                Some(format!(
                    "the {iface} desktop portal is version {version}, but this tool needs version {min_ver}"
                )),
            ),
            None => (
                false,
                Some(format!("the {iface} desktop portal is not available")),
            ),
        },
    }
}

pub fn report(state: &AppState) -> CapabilitiesReport {
    let mut binaries = BTreeMap::new();
    let tools = state
        .registry
        .list()
        .iter()
        .map(|descriptor| {
            if let Requires::Binary(binary) = &descriptor.requires {
                binaries.insert(binary.clone(), state.availability.binaries.contains(binary));
            }
            let (available, reason) = is_available(&state.availability, &descriptor.requires);
            ToolAvailability {
                id: descriptor.id.as_ref().to_owned(),
                title: descriptor.title.clone(),
                available,
                reason,
            }
        })
        .collect();
    CapabilitiesReport {
        binaries,
        portals: state
            .availability
            .portals
            .iter()
            .map(|(name, version)| (name.clone(), *version))
            .collect(),
        tools,
    }
}
