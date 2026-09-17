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
/// `All` reports its first unmet entry, same order as `wield-core`'s own
/// `check_requires` - keeps this UI-facing reason consistent with the
/// execution-time gate.
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
        Requires::All(all) => all
            .iter()
            .map(|inner| is_available(availability, inner))
            .find(|(available, _)| !available)
            .unwrap_or((true, None)),
    }
}

pub fn report(state: &AppState) -> CapabilitiesReport {
    let mut binaries = BTreeMap::new();
    let tools = state
        .registry
        .list()
        .iter()
        .map(|descriptor| {
            for binary in binary_requirements(&descriptor.requires) {
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

/// Every `Binary` requirement named anywhere inside `requires`, including
/// nested inside `All` - `report()`'s diagnostics map needs all of them,
/// not just a direct top-level `Requires::Binary`.
fn binary_requirements(requires: &Requires) -> Vec<&String> {
    match requires {
        Requires::Binary(binary) => vec![binary],
        Requires::All(all) => all.iter().flat_map(binary_requirements).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn availability_with(binaries: &[&str], portals: &[(&str, u32)]) -> AvailabilityView {
        let mut view = AvailabilityView::default();
        for binary in binaries {
            view.binaries.insert((*binary).to_owned());
        }
        for (iface, version) in portals {
            view.portals.insert((*iface).to_owned(), *version);
        }
        view
    }

    fn screen_ocr_requires() -> Requires {
        Requires::All(vec![
            Requires::Portal {
                iface: "Screenshot".into(),
                min_ver: 2,
            },
            Requires::Binary("tesseract".into()),
        ])
    }

    #[test]
    fn all_reports_the_first_unmet_entry() {
        let (available, reason) =
            is_available(&availability_with(&[], &[]), &screen_ocr_requires());
        assert!(!available);
        assert_eq!(
            reason.as_deref(),
            Some("the Screenshot desktop portal is not available")
        );
    }

    #[test]
    fn all_is_available_only_when_every_entry_is() {
        let view = availability_with(&["tesseract"], &[("Screenshot", 2)]);
        let (available, reason) = is_available(&view, &screen_ocr_requires());
        assert!(available);
        assert!(reason.is_none());
    }

    #[test]
    fn binary_requirements_collects_nested_entries() {
        let requires = screen_ocr_requires();
        let names: Vec<&str> = binary_requirements(&requires)
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["tesseract"]);
    }
}
