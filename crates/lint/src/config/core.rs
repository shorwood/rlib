use super::schema::FileConfig;
use super::validation;

// -----------------------------------------------------------------------------
// BooleanPredicateConfig: Boolean property and query vocabulary
// -----------------------------------------------------------------------------

/// Property prefixes shared by boolean fields and boolean-result callables.
const BOOLEAN_PREDICATE_PREFIXES: &[&str] = &["is_", "has_", "should_"];

/// Canonical Rust query roots accepted only for boolean-result callables.
const BOOLEAN_QUERY_ROOTS: &[&str] = &[
    "all",
    "any",
    "contains",
    "ends_with",
    "exists",
    "starts_with",
    "try_exists",
];

/// Validated naming policy for boolean properties and callable results.
#[derive(Clone)]
pub struct BooleanPredicateConfig {
    /// Prefixes that introduce a nonempty predicate phrase.
    property_prefixes: Vec<String>,
    /// Complete or underscore-extended query roots accepted for callables.
    query_roots: Vec<String>,
}

impl BooleanPredicateConfig {
    /// Returns whether text is a canonical nonempty snake-case identifier fragment.
    fn is_snake_fragment(value: &str) -> bool {
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            && value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && !value.contains("__")
    }

    /// Validates one property prefix such as `is_`.
    fn property_prefix(key: &str, value: &str) -> Result<(), String> {
        // A missing delimiter cannot introduce a predicate phrase unambiguously.
        let Some(stem) = value.strip_suffix('_') else {
            return Err(format!("{key} entry `{value}` must end with `_`"));
        };

        // Reject shapes that would produce malformed identifiers after concatenation.
        if !Self::is_snake_fragment(stem) || stem.ends_with('_') {
            return Err(format!(
                "{key} entry `{value}` must be a canonical snake-case prefix"
            ));
        }
        Ok(())
    }

    /// Validates one callable query root such as `contains`.
    fn query_root(key: &str, value: &str) -> Result<(), String> {
        // Query roots are complete names rather than property-prefix fragments.
        if !Self::is_snake_fragment(value) || value.ends_with('_') {
            return Err(format!(
                "{key} entry `{value}` must be a canonical snake-case name without a trailing underscore"
            ));
        }
        Ok(())
    }

    /// Renders a readable list of configuration-backed naming forms.
    fn list(forms: &[String]) -> String {
        match forms {
            [] => String::new(),
            [only] => only.clone(),
            [first, second] => format!("{first} or {second}"),
            _ => {
                let (last, rest) = forms.split_last().expect("nonempty forms have a last item");
                format!("{}, or {last}", rest.join(", "))
            }
        }
    }

    /// Describes the configured field naming forms for diagnostics.
    pub(crate) fn field_expectation(&self) -> String {
        Self::list(
            &self
                .property_prefixes
                .iter()
                .map(|prefix| format!("`{prefix}<predicate>`"))
                .collect::<Vec<_>>(),
        )
    }

    /// Describes the configured callable naming forms for diagnostics.
    pub(crate) fn callable_expectation(&self) -> String {
        match (
            self.property_prefixes.is_empty(),
            self.query_roots.is_empty(),
        ) {
            (false, false) => "a configured predicate prefix or query root".to_owned(),
            (false, true) => "a configured predicate prefix".to_owned(),
            (true, false) => "a configured query root".to_owned(),
            (true, true) => "the configured boolean naming policy".to_owned(),
        }
    }

    /// Returns the nonempty predicate phrase following one property prefix.
    fn property_phrase<'name>(&self, name: &'name str) -> Option<&'name str> {
        self.property_prefixes.iter().find_map(|prefix| {
            name.strip_prefix(prefix)
                .filter(|predicate| Self::is_snake_fragment(predicate))
        })
    }

    /// Returns whether a field name uses one configured property prefix.
    pub(crate) fn is_field_name(&self, name: &str) -> bool {
        self.property_phrase(name).is_some()
    }

    /// Returns whether a callable name uses a property prefix or canonical query root.
    pub(crate) fn is_callable_name(&self, name: &str) -> bool {
        self.is_field_name(name)
            || self.query_roots.iter().any(|root| {
                name == root
                    || name
                        .strip_prefix(root)
                        .and_then(|suffix| suffix.strip_prefix('_'))
                        .is_some_and(Self::is_snake_fragment)
            })
    }

    /// Removes leading callable grammar so ownership analysis sees the semantic subject.
    pub(crate) fn semantic_callable_name<'name>(&self, name: &'name str) -> &'name str {
        self.property_phrase(name)
            .or_else(|| {
                self.query_roots.iter().find_map(|root| {
                    name.strip_prefix(root)
                        .and_then(|suffix| suffix.strip_prefix('_'))
                        .filter(|subject| Self::is_snake_fragment(subject))
                })
            })
            .unwrap_or(name)
    }
}

impl TryFrom<&FileConfig> for BooleanPredicateConfig {
    type Error = String;

    fn try_from(file: &FileConfig) -> Result<Self, Self::Error> {
        let property_key = "boolean-predicate-prefixes";
        let query_key = "boolean-query-roots";
        let property_prefixes = validation::ConfigList::resolve(
            property_key,
            file.boolean_predicate_prefixes.clone(),
            BOOLEAN_PREDICATE_PREFIXES,
        )?;

        // Fields require at least one structural predicate form.
        if property_prefixes.is_empty() {
            return Err(format!("{property_key} must contain at least one prefix"));
        }
        for prefix in &property_prefixes {
            Self::property_prefix(property_key, prefix)?;
        }
        let query_roots = validation::ConfigList::resolve(
            query_key,
            file.boolean_query_roots.clone(),
            BOOLEAN_QUERY_ROOTS,
        )?;
        for root in &query_roots {
            Self::query_root(query_key, root)?;
        }

        // Keep the two namespaces structurally distinct and diagnostics unambiguous.
        if let Some(root) = query_roots.iter().find(|root| {
            property_prefixes
                .iter()
                .any(|prefix| prefix == &format!("{root}_"))
        }) {
            return Err(format!(
                "{query_key} entry `{root}` duplicates a configured property prefix"
            ));
        }
        Ok(Self {
            property_prefixes,
            query_roots,
        })
    }
}

// -----------------------------------------------------------------------------
// FunctionStructureConfig: Function complexity policy
// -----------------------------------------------------------------------------

/// Limits and fixed syntax shared by function-structure lints.
#[derive(Clone)]
#[expect(
    clippy::struct_field_names,
    reason = "the shared maximum prefix distinguishes enforced limits from measured values"
)]
pub struct FunctionStructureConfig {
    /// Maximum lines in one unnamed code phase.
    pub(crate) max_phase_lines: usize,
    /// Maximum nested control-flow depth.
    pub(crate) max_control_flow_depth: usize,
    /// Maximum lines in one match arm.
    pub(crate) max_match_arm_lines: usize,
    /// Maximum calls in one method chain.
    pub(crate) max_method_chain_calls: usize,
}

impl FunctionStructureConfig {
    /// Fixed prefix recognized for code-phase comments.
    pub(crate) const PHASE_COMMENT_PREFIX: &str = "//";
}

// -----------------------------------------------------------------------------
// SectionDividerConfig: Module section policy
// -----------------------------------------------------------------------------

/// Canonical source template for module section dividers.
const SECTION_DIVIDER_TEMPLATE: &str = "// -----------------------------------------------------------------------------\n\
     // {content}\n\
     // -----------------------------------------------------------------------------";

/// Limits and fixed syntax shared by module-section lints.
#[derive(Clone)]
pub struct SectionDividerConfig {
    /// Maximum declarations allowed in one authored section.
    pub(crate) max_declarations_per_section: usize,
}

impl SectionDividerConfig {
    /// Canonical three-line section divider template.
    pub(crate) const TEMPLATE: &str = SECTION_DIVIDER_TEMPLATE;

    /// Maximum accepted divider line width.
    pub(crate) const MAX_LINE_LENGTH: usize = 80;
}

// -----------------------------------------------------------------------------
// ExtensionTraitConfig: Extension trait policy
// -----------------------------------------------------------------------------

/// Limits shared by extension-trait lints.
#[derive(Clone)]
pub struct ExtensionTraitConfig {
    /// Maximum methods in one coherent extension trait.
    pub(crate) max_methods: usize,
}
