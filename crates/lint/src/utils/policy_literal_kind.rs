use super::identifier_case;

// -----------------------------------------------------------------------------
// PolicyBoundaryVocabulary: Retry, timing, and threshold evidence
// -----------------------------------------------------------------------------

/// Vocabulary that proves a repeated-operation policy.
const POLICY_BOUNDARY_VOCABULARY_RETRY: &[&str] =
    &["attempt", "attempts", "retry", "retries", "backoff"];
/// Vocabulary that proves a time-based policy.
const POLICY_BOUNDARY_VOCABULARY_TIMING: &[&str] = &[
    "delay",
    "timeout",
    "expiry",
    "expiration",
    "ttl",
    "interval",
    "duration",
];
/// Vocabulary that proves a general behavioral bound.
const POLICY_BOUNDARY_VOCABULARY_THRESHOLD: &[&str] = &[
    "threshold",
    "limit",
    "maximum",
    "minimum",
    "max",
    "min",
    "budget",
];

// -----------------------------------------------------------------------------
// PolicyResourceVocabulary: Capacity and concurrency evidence
// -----------------------------------------------------------------------------

/// Vocabulary that proves a resource-sizing policy.
const POLICY_RESOURCE_VOCABULARY_CAPACITY: &[&str] = &[
    "capacity", "buffer", "queue", "stack", "batch", "chunk", "size",
];
/// Vocabulary that proves a parallel-execution policy.
const POLICY_RESOURCE_VOCABULARY_CONCURRENCY: &[&str] = &[
    "concurrency",
    "parallelism",
    "permit",
    "permits",
    "worker",
    "workers",
    "thread",
    "threads",
];
// -----------------------------------------------------------------------------
// PolicyOutputVocabulary: Pagination and truncation evidence
// -----------------------------------------------------------------------------

/// Vocabulary that proves a paging policy.
const POLICY_OUTPUT_VOCABULARY_PAGINATION: &[&str] = &["page", "pages", "offset", "pagination"];
/// Vocabulary that proves a retained-length policy.
const POLICY_OUTPUT_VOCABULARY_TRUNCATION: &[&str] = &["truncate", "truncation", "retained"];

// -----------------------------------------------------------------------------
// PolicyCategory: Operational policy dimensions
// -----------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Behavioral dimension controlled by a literal-backed value.
#[repr(usize)]
pub enum PolicyCategory {
    /// Retry count or repeated fallible execution bound.
    Retry,
    /// Delay, timeout, expiry, or other duration.
    Timing,
    /// Allocation, queue, buffer, or stack capacity.
    Capacity,
    /// Worker, permit, or concurrent operation count.
    Concurrency,
    /// Page size, offset, or iterator cardinality.
    Pagination,
    /// Maximum retained collection or string length.
    Truncation,
    /// Branch threshold or general operational limit.
    Threshold,
}

/// Every category in diagnostic precedence order.
const POLICY_CATEGORY_ALL: [PolicyCategory; 7] = [
    PolicyCategory::Retry,
    PolicyCategory::Timing,
    PolicyCategory::Concurrency,
    PolicyCategory::Pagination,
    PolicyCategory::Truncation,
    PolicyCategory::Capacity,
    PolicyCategory::Threshold,
];

/// Identifier vocabulary indexed by the category discriminant.
const POLICY_CATEGORY_VOCABULARIES: [&[&str]; 7] = [
    POLICY_BOUNDARY_VOCABULARY_RETRY,
    POLICY_BOUNDARY_VOCABULARY_TIMING,
    POLICY_RESOURCE_VOCABULARY_CAPACITY,
    POLICY_RESOURCE_VOCABULARY_CONCURRENCY,
    POLICY_OUTPUT_VOCABULARY_PAGINATION,
    POLICY_OUTPUT_VOCABULARY_TRUNCATION,
    POLICY_BOUNDARY_VOCABULARY_THRESHOLD,
];

impl PolicyCategory {
    /// Every category in diagnostic precedence order.
    pub(super) const fn all() -> [Self; 7] {
        // Preserve the same precedence used when overlapping evidence is merged.
        POLICY_CATEGORY_ALL
    }

    /// User-facing noun phrase used by diagnostics.
    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::Retry => "retry bound",
            Self::Timing => "timing policy",
            Self::Capacity => "capacity policy",
            Self::Concurrency => "concurrency policy",
            Self::Pagination => "pagination policy",
            Self::Truncation => "truncation policy",
            Self::Threshold => "behavioral threshold",
        }
    }

    /// Returns whether one normalized identifier word names this policy category.
    pub(super) fn matches_word(self, word: &str) -> bool {
        // Select by discriminant so the vocabulary lookup remains uniform.
        POLICY_CATEGORY_VOCABULARIES[self as usize].contains(&word)
    }

    /// Selects the strongest policy category implied by an authored identifier.
    pub(super) fn from_name(name: &str) -> Option<Self> {
        // Normalize once before testing the ordered high-confidence vocabulary families.
        let words = identifier_case::words(name)
            .into_iter()
            .map(|word| word.to_ascii_lowercase())
            .collect::<Vec<_>>();
        Self::all()
            .into_iter()
            .find(|category| words.iter().any(|word| category.matches_word(word)))
    }

    /// Ranking used when one literal receives overlapping evidence.
    pub(super) const fn evidence_rank(self) -> u8 {
        match self {
            Self::Retry => 7,
            Self::Timing => 6,
            Self::Concurrency => 5,
            Self::Capacity => 4,
            Self::Pagination => 3,
            Self::Truncation => 2,
            Self::Threshold => 1,
        }
    }
}
