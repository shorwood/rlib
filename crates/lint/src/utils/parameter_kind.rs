extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::DefId;

// -----------------------------------------------------------------------------
// Parameter: Interchangeable primitive representations
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

    /// Classifies exact numeric primitive representations.
    fn numeric(ty: Ty<'_>) -> Option<Self> {
        match ty.kind() {
            ty::Int(representation) => Some(Self::signed(*representation)),
            ty::Uint(representation) => Some(Self::unsigned(*representation)),
            ty::Float(representation) => Some(Self::float(*representation)),
            _ => None,
        }
    }

    /// Classifies scalar primitive and directly borrowed textual representations.
    fn scalar(ty: Ty<'_>) -> Option<Self> {
        if let Some(kind) = Self::numeric(ty) {
            return Some(kind);
        }
        match ty.kind() {
            ty::Char => Some(Self::Character),
            ty::Str => Some(Self::Text),
            _ => None,
        }
    }

    /// Classifies standard textual ADTs after resolving re-exported definition paths.
    fn adt(cx: &LateContext<'_>, def_id: DefId, arguments: ty::GenericArgsRef<'_>) -> Option<Self> {
        let path = cx.tcx.def_path_str(def_id);
        if path.ends_with("::string::String") {
            return Some(Self::Text);
        }
        if !path.ends_with("::boxed::Box") && !path.ends_with("::borrow::Cow") {
            return None;
        }
        arguments
            .types()
            .any(|argument| matches!(argument.kind(), ty::Str))
            .then_some(Self::Text)
    }

    /// Describes this family in a diagnostic without exposing compiler terminology.
    pub(crate) fn description(self) -> String {
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

/// Parameter-family queries colocated with compiler types.
pub(super) trait ParameterTypeExt {
    /// Classifies aliases and references by their interchangeable representation.
    fn interchangeable_kind(self, cx: &LateContext<'_>) -> Option<ParameterKind>;

    /// Returns whether this resolved type belongs to the textual family.
    fn is_textual(&self, cx: &LateContext<'_>) -> bool;
}

impl ParameterTypeExt for Ty<'_> {
    fn interchangeable_kind(self, cx: &LateContext<'_>) -> Option<ParameterKind> {
        // References do not distinguish semantic roles at the call site.
        let ty = self.peel_refs();

        // Resolve scalar values before inspecting standard-library text containers.
        if let Some(kind) = ParameterKind::scalar(ty) {
            return Some(kind);
        }
        let ty::Adt(definition, arguments) = ty.kind() else {
            return None;
        };
        ParameterKind::adt(cx, definition.did(), arguments)
    }

    fn is_textual(&self, cx: &LateContext<'_>) -> bool {
        (*self).interchangeable_kind(cx) == Some(ParameterKind::Text)
    }
}
