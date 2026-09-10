//! The tool registry: registration with validation, lookup, fuzzy search, and
//! availability filtering.

use crate::descriptor::{Descriptor, Requires};
use crate::error::DescriptorError;
use crate::executor::AvailabilityView;
use crate::validate::validate_descriptor;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// Why a descriptor was rejected at registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A tool with this `id` is already registered.
    DuplicateId(String),
    /// The descriptor failed `validate_descriptor`.
    Invalid {
        id: String,
        errors: Vec<DescriptorError>,
    },
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate tool id: {id}"),
            Self::Invalid { id, errors } => {
                write!(f, "invalid descriptor {id} ({} problems)", errors.len())
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// An ordered set of tool descriptors. Registration order is preserved for
/// blank-query search results.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    tools: Vec<Descriptor>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate `descriptor` and add it. Rejects a duplicate `id` and any
    /// descriptor that fails `validate_descriptor`.
    pub fn register(&mut self, descriptor: Descriptor) -> Result<(), RegistryError> {
        let id = descriptor.id.as_ref().to_owned();
        if self.get(&id).is_some() {
            return Err(RegistryError::DuplicateId(id));
        }
        if let Err(errors) = validate_descriptor(&descriptor) {
            return Err(RegistryError::Invalid { id, errors });
        }
        self.tools.push(descriptor);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn list(&self) -> &[Descriptor] {
        &self.tools
    }

    pub fn get(&self, id: &str) -> Option<&Descriptor> {
        self.tools.iter().find(|tool| tool.id.as_ref() == id)
    }

    /// Fuzzy-search over each tool's `title` and `keywords`. A blank query
    /// returns every tool in registration order. Otherwise the query is split
    /// into whitespace-separated terms; a tool matches only if every term
    /// fuzzy-matches its title or one of its keywords. Results are ordered by
    /// descending total score then ascending `id`.
    pub fn search(&self, query: &str) -> Vec<&Descriptor> {
        let query = query.trim();
        if query.is_empty() {
            return self.tools.iter().collect();
        }

        let matcher = SkimMatcherV2::default();
        let terms: Vec<&str> = query.split_whitespace().collect();
        let mut scored: Vec<(i64, &Descriptor)> = self
            .tools
            .iter()
            .filter_map(|tool| {
                let haystacks: Vec<&str> = std::iter::once(tool.title.as_str())
                    .chain(tool.keywords.iter().map(String::as_str))
                    .collect();
                let mut total = 0_i64;
                for term in &terms {
                    let best = haystacks
                        .iter()
                        .filter_map(|haystack| matcher.fuzzy_match(haystack, term))
                        .max()?;
                    total += best;
                }
                Some((total, tool))
            })
            .collect();

        scored.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .cmp(left_score)
                .then_with(|| left.id.as_ref().cmp(right.id.as_ref()))
        });
        scored.into_iter().map(|(_, tool)| tool).collect()
    }

    /// The subset of tools whose `requires` is satisfied by `view`.
    pub fn available<'a>(&'a self, view: &AvailabilityView) -> Vec<&'a Descriptor> {
        self.tools
            .iter()
            .filter(|tool| match &tool.requires {
                Requires::None => true,
                Requires::Binary(binary) => view.binaries.contains(binary),
                Requires::Portal { iface, min_ver } => {
                    view.portals.get(iface).is_some_and(|version| version >= min_ver)
                }
            })
            .collect()
    }

    /// A deterministic JSON snapshot of every registered descriptor, sorted by
    /// `id`. Stable across runs — used by the descriptor snapshot test.
    pub fn snapshot(&self) -> String {
        let mut sorted = self.tools.clone();
        sorted.sort_by(|left, right| left.id.as_ref().cmp(right.id.as_ref()));
        serde_json::to_string_pretty(&sorted).expect("descriptors serialize")
    }
}
