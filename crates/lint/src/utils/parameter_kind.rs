extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};

// -----------------------------------------------------------------------------
// ParameterKind: Interchangeable primitive representations
// -----------------------------------------------------------------------------

/// Primitive families whose values remain interchangeable at a call site.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParameterKind {
    /// One signed integer representation.
    SignedInteger {
        /// Exact compiler-resolved signed integer type.
        representation: ty::IntTy,
    },
    /// One unsigned integer representation.
    UnsignedInteger {
        /// Exact compiler-resolved unsigned integer type.
        representation: ty::UintTy,
    },
    /// One floating-point representation.
    Float {
        /// Exact compiler-resolved floating-point type.
        representation: ty::FloatTy,
    },
    /// Rust character values.
    Character,
    /// Owned or borrowed UTF-8 text.
    Text,
}

impl ParameterKind {
    /// Constructs one exact signed-integer family.
    const fn signed(representation: ty::IntTy) -> Self {
        Self::SignedInteger { representation }
    }

    /// Constructs one exact unsigned-integer family.
    const fn unsigned(representation: ty::UintTy) -> Self {
        Self::UnsignedInteger { representation }
    }

    /// Constructs one exact floating-point family.
    const fn float(representation: ty::FloatTy) -> Self {
        Self::Float { representation }
    }

    /// Describes this family in a diagnostic without exposing compiler terminology.
    pub fn description(self) -> String {
        match self {
            Self::SignedInteger { representation } => {
                format!("`{representation:?}`").to_lowercase()
            }
            Self::UnsignedInteger { representation } => {
                format!("`{representation:?}`").to_lowercase()
            }
            Self::Float { representation } => format!("`{representation:?}`").to_lowercase(),
            Self::Character => "`char`".to_owned(),
            Self::Text => "textual".to_owned(),
        }
    }
}

/// Classifies exact numeric primitive representations.
fn numeric_kind(ty: Ty<'_>) -> Option<ParameterKind> {
    match ty.kind() {
        ty::Int(representation) => Some(ParameterKind::signed(*representation)),
        ty::Uint(representation) => Some(ParameterKind::unsigned(*representation)),
        ty::Float(representation) => Some(ParameterKind::float(*representation)),
        _ => None,
    }
}

/// Classifies scalar primitive and directly borrowed textual representations.
fn scalar_kind(ty: Ty<'_>) -> Option<ParameterKind> {
    if let Some(kind) = numeric_kind(ty) {
        return Some(kind);
    }
    match ty.kind() {
        ty::Char => Some(ParameterKind::Character),
        ty::Str => Some(ParameterKind::Text),
        _ => None,
    }
}

/// Classifies standard textual ADTs after resolving re-exported definition paths.
fn adt_kind(
    cx: &LateContext<'_>,
    def_id: rustc_span::def_id::DefId,
    arguments: ty::GenericArgsRef<'_>,
) -> Option<ParameterKind> {
    let path = cx.tcx.def_path_str(def_id);
    if path.ends_with("::string::String") {
        return Some(ParameterKind::Text);
    }
    if !path.ends_with("::boxed::Box") && !path.ends_with("::borrow::Cow") {
        return None;
    }
    arguments
        .types()
        .any(|argument| matches!(argument.kind(), ty::Str))
        .then_some(ParameterKind::Text)
}

/// Classifies aliases and references by their interchangeable representation.
pub(super) fn interchangeable_kind(cx: &LateContext<'_>, ty: Ty<'_>) -> Option<ParameterKind> {
    // References do not distinguish semantic roles at the call site.
    let ty = ty.peel_refs();

    // Resolve scalar values before inspecting standard-library text containers.
    if let Some(kind) = scalar_kind(ty) {
        return Some(kind);
    }
    let ty::Adt(definition, arguments) = ty.kind() else {
        return None;
    };
    adt_kind(cx, definition.did(), arguments)
}

/// Returns whether a resolved type belongs to the textual family.
pub fn type_is_textual(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    interchangeable_kind(cx, ty) == Some(ParameterKind::Text)
}
