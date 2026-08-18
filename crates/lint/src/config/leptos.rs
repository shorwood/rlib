#![cfg_attr(not(feature = "leptos"), allow(dead_code))]

#[cfg(feature = "leptos")]
use leptosfmt_formatter::{
    AttributeValueBraceStyle, ClosingTagStyle, FormatterSettings, IndentationStyle, NewlineStyle,
};
#[cfg(feature = "leptos_styling")]
use malva::config::{FormatOptions, LineBreak};

use super::schema::FileConfig;
use super::validation;

// -----------------------------------------------------------------------------
// LeptosArchitectureConfig: Component architecture limits
// -----------------------------------------------------------------------------

/// Validated limits shared by Leptos component architecture lints.
#[derive(Clone)]
pub struct LeptosArchitectureConfig {
    /// Maximum setup statements before a view.
    pub(crate) max_setup_statements: usize,
    /// Maximum reactive primitives in one function.
    pub(crate) max_reactive_primitives: usize,
    /// Maximum statements in one event handler.
    pub(crate) max_handler_statements: usize,
    /// Maximum event-handler control-flow depth.
    pub(crate) max_handler_control_flow_depth: usize,
    /// Maximum nested view element depth.
    pub(crate) max_view_nesting_depth: usize,
    /// Maximum view control-flow depth.
    pub(crate) max_view_control_depth: usize,
    /// Maximum local component composition depth.
    pub(crate) max_component_composition_depth: usize,
    /// Maximum non-children component props.
    pub(crate) max_component_props: usize,
    /// Maximum components in one module.
    pub(crate) max_components_per_module: usize,
    /// Minimum nodes in a repeated fragment candidate.
    pub(crate) min_repeated_fragment_nodes: usize,
    /// Minimum occurrences of a repeated fragment candidate.
    pub(crate) min_repeated_fragment_occurrences: usize,
}

// -----------------------------------------------------------------------------
// LeptosViewStructureConfig: View heading and group limits
// -----------------------------------------------------------------------------

/// Validated complexity limits and fixed syntax for authored Leptos views.
#[derive(Clone)]
#[expect(
    clippy::struct_field_names,
    reason = "the shared maximum prefix distinguishes enforced limits from measured values"
)]
pub struct LeptosViewStructureConfig {
    /// Maximum complexity before an unnamed view needs sections.
    pub(crate) max_unnamed_view_complexity: usize,
    /// Maximum complexity in one named view section.
    pub(crate) max_view_section_complexity: usize,
    /// Maximum complexity before attributes need groups.
    pub(crate) max_unnamed_view_attribute_complexity: usize,
    /// Maximum complexity in one named attribute group.
    pub(crate) max_view_attribute_group_complexity: usize,
}

impl LeptosViewStructureConfig {
    /// Fixed prefix recognized for view section and attribute group comments.
    pub(crate) const VIEW_SECTION_COMMENT_PREFIX: &str = "//";
}

// -----------------------------------------------------------------------------
// LeptosViewFormattingConfig: View formatter policy
// -----------------------------------------------------------------------------

/// Formatter policy for canonical Leptos `view!` source.
#[derive(Clone)]
pub struct LeptosViewFormattingConfig {
    /// Maximum formatted view width.
    pub(super) max_width: usize,
}

#[cfg(feature = "leptos")]
impl LeptosViewFormattingConfig {
    /// Builds deterministic `leptosfmt` settings from project policy.
    pub(crate) fn settings(&self) -> FormatterSettings {
        FormatterSettings {
            max_width: self.max_width,
            tab_spaces: 4,
            indentation_style: IndentationStyle::Spaces,
            newline_style: NewlineStyle::Unix,
            attr_value_brace_style: AttributeValueBraceStyle::WhenRequired,
            closing_tag_style: ClosingTagStyle::Preserve,
            ..FormatterSettings::default()
        }
    }
}

// -----------------------------------------------------------------------------
// LeptosStylingCssFormattingConfig: CSS formatter policy
// -----------------------------------------------------------------------------

/// Formatter policy for canonical component CSS.
#[derive(Clone)]
#[cfg_attr(not(feature = "leptos_styling"), allow(dead_code))]
pub struct LeptosStylingCssFormattingConfig {
    /// Maximum formatted CSS width.
    pub(super) max_width: usize,
}

#[cfg(feature = "leptos_styling")]
impl LeptosStylingCssFormattingConfig {
    /// Builds deterministic Malva options from project policy.
    pub(crate) fn options(&self) -> FormatOptions {
        let mut options = FormatOptions::default();
        options.layout.print_width = self.max_width;
        options.layout.use_tabs = false;
        options.layout.indent_width = 2;
        options.layout.line_break = LineBreak::Lf;
        options.language.declaration_order = None;
        options
    }
}

// -----------------------------------------------------------------------------
// LeptosServerAuthorizationConfig: Server endpoint policy
// -----------------------------------------------------------------------------

/// Default terms that make a server-side call authorization-sensitive.
const SENSITIVE_CALL_TERMS: &[&str] = &[
    "create",
    "delete",
    "remove",
    "update",
    "upload",
    "credential",
    "private",
    "admin",
];

/// Configurable vocabulary used to identify protected Leptos server endpoints.
#[derive(Clone)]
pub struct LeptosServerAuthorizationConfig {
    /// Terms that classify a call as authorization-sensitive.
    pub(crate) sensitive_call_terms: Vec<String>,
    /// Functions recognized as authorization boundaries.
    pub(crate) authorization_functions: Vec<String>,
    /// Attributes that explicitly mark protected endpoints.
    pub(crate) protected_endpoint_attributes: Vec<String>,
    /// Attributes that explicitly mark public endpoints.
    pub(crate) public_endpoint_attributes: Vec<String>,
}

impl TryFrom<&FileConfig> for LeptosServerAuthorizationConfig {
    type Error = String;

    fn try_from(file: &FileConfig) -> Result<Self, Self::Error> {
        // Resolve sensitive terms with their built-in defaults.
        let sensitive_call_terms = validation::ConfigList::resolve(
            "leptos-sensitive-call-terms",
            file.leptos_sensitive_call_terms.clone(),
            SENSITIVE_CALL_TERMS,
        )?;

        // Resolve explicit authorization boundary functions.
        let authorization_functions = validation::ConfigList::resolve(
            "leptos-authorization-functions",
            file.leptos_authorization_functions.clone(),
            &[],
        )?;

        // Resolve attributes that mark protected server endpoints.
        let protected_endpoint_attributes = validation::ConfigList::resolve(
            "leptos-protected-endpoint-attributes",
            file.leptos_protected_endpoint_attributes.clone(),
            &[],
        )?;

        // Resolve attributes that mark intentionally public server endpoints.
        let public_endpoint_attributes = validation::ConfigList::resolve(
            "leptos-public-endpoint-attributes",
            file.leptos_public_endpoint_attributes.clone(),
            &[],
        )?;

        Ok(Self {
            sensitive_call_terms,
            authorization_functions,
            protected_endpoint_attributes,
            public_endpoint_attributes,
        })
    }
}
