extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::LateContext;
use rustc_span::Symbol;
use rustc_span::def_id::DefId;

use super::policy_literal_kind::PolicyCategory;

// -----------------------------------------------------------------------------
// PolicyApiStandard: Standard library method families
// -----------------------------------------------------------------------------

/// Standard duration constructors whose numeric arguments define timing policy.
const POLICY_API_STANDARD_DURATION_CONSTRUCTOR_NAMES: &[&str] = &[
    "new",
    "from_secs",
    "from_millis",
    "from_micros",
    "from_nanos",
    "from_nanos_u128",
    "from_secs_f32",
    "from_secs_f64",
    "try_from_secs_f32",
    "try_from_secs_f64",
];

/// Standard collection APIs whose argument reserves resources.
const POLICY_API_STANDARD_COLLECTION_CAPACITY_NAMES: &[&str] = &[
    "with_capacity",
    "with_capacity_in",
    "with_capacity_and_hasher",
    "reserve",
    "reserve_exact",
    "try_reserve",
    "try_reserve_exact",
];

/// Definition-path segments for standard growable collections.
const POLICY_API_STANDARD_COLLECTION_PATH_SEGMENTS: &[&str] = &[
    "::Vec::",
    "::VecDeque::",
    "::String::",
    "::HashMap::",
    "::HashSet::",
    "::BinaryHeap::",
];

// -----------------------------------------------------------------------------
// PolicyApiEcosystem: Dependency method families
// -----------------------------------------------------------------------------

/// Tokio runtime-builder methods whose argument bounds scheduler resources.
const POLICY_API_ECOSYSTEM_TOKIO_RUNTIME_NAMES: &[&str] = &[
    "worker_threads",
    "max_blocking_threads",
    "thread_stack_size",
    "global_queue_interval",
    "event_interval",
    "max_io_events_per_tick",
];

/// Futures extension methods whose argument bounds concurrent work.
const POLICY_API_ECOSYSTEM_FUTURES_CONCURRENCY_NAMES: &[&str] = &[
    "buffered",
    "buffer_unordered",
    "for_each_concurrent",
    "try_buffered",
    "try_buffer_unordered",
    "try_for_each_concurrent",
];

/// Ecosystem crates whose `bounded` constructor sets channel capacity.
const POLICY_API_ECOSYSTEM_BOUNDED_CHANNEL_CRATES: &[&str] =
    &["crossbeam_channel", "async_channel", "flume"];

// -----------------------------------------------------------------------------
// PolicyCall: Resolved call site
// -----------------------------------------------------------------------------

/// Definition and authored argument count for one call expression.
struct PolicyCall {
    /// Resolved function or associated-item definition.
    definition: DefId,
    /// Number of non-receiver arguments at the call site.
    argument_count: usize,
}

impl PolicyCall {
    /// Resolves direct function syntax after the callee has been identified.
    fn resolve_direct(
        cx: &LateContext<'_>,
        callee: &Expr<'_>,
        argument_count: usize,
    ) -> Option<Self> {
        // Require a definition-backed path rather than an arbitrary callable expression.
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };

        // Resolve the path to the concrete function definition.
        let Res::Def(_, definition) = cx.qpath_res(&path, callee.hir_id) else {
            return None;
        };
        Some(Self {
            definition,
            argument_count,
        })
    }

    /// Resolves direct and method call syntax into one named representation.
    fn resolve(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Self> {
        // Resolve the definition according to the call syntax selected by HIR.
        match expression.kind {
            ExprKind::Call(callee, arguments) => Self::resolve_direct(cx, callee, arguments.len()),
            ExprKind::MethodCall(_, _, arguments, _) => {
                let definition = cx
                    .typeck_results()
                    .type_dependent_def_id(expression.hir_id)?;
                Some(Self {
                    definition,
                    argument_count: arguments.len(),
                })
            }
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// PolicyApiIdentity: Resolved api vocabulary
// -----------------------------------------------------------------------------

/// Semantic identity used to classify one resolved call without raw string parameter families.
struct PolicyApiIdentity {
    /// Defining crate rather than the source-level reexport crate.
    crate_name: Symbol,
    /// Compiler-rendered definition path.
    path: String,
    /// Resolved item name.
    item_name: Symbol,
    /// Number of non-receiver arguments at the call site.
    argument_count: usize,
}

impl PolicyApiIdentity {
    /// Captures stable classification vocabulary from one resolved call.
    fn resolve(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Self> {
        // Resolve the call before materializing its defining identity once.
        let call = PolicyCall::resolve(cx, expression)?;
        Some(Self {
            crate_name: cx.tcx.crate_name(call.definition.krate),
            path: cx.tcx.def_path_str(call.definition),
            item_name: cx.tcx.item_name(call.definition),
            argument_count: call.argument_count,
        })
    }

    /// Returns whether this identity belongs to the standard distribution.
    fn is_standard(&self) -> bool {
        matches!(self.crate_name.as_str(), "core" | "alloc" | "std")
    }

    /// Returns whether this identity belongs to a standard growable collection.
    fn is_standard_collection(&self) -> bool {
        // Require both a standard defining crate and a recognized collection path.
        self.is_standard()
            && POLICY_API_STANDARD_COLLECTION_PATH_SEGMENTS
                .iter()
                .any(|segment| self.path.contains(segment))
    }
}

// -----------------------------------------------------------------------------
// PolicyApi: Curated policy sink
// -----------------------------------------------------------------------------
/// Category and positions supplied by one catalogue family.
#[derive(Clone, Copy)]
struct PolicyApiShape {
    /// Operational policy category.
    category: PolicyCategory,
    /// Non-receiver arguments that carry policy values.
    argument_positions: &'static [usize],
}

impl PolicyApiShape {
    /// Creates the common shape whose first non-receiver argument carries policy.
    const fn single(category: PolicyCategory) -> Self {
        Self {
            category,
            argument_positions: &[0],
        }
    }
}

/// One resolved API whose selected arguments encode operational policy.
pub(super) struct PolicyApi {
    /// Kind of policy controlled by the selected arguments.
    pub(super) category: PolicyCategory,
    /// Stable user-facing API description used by diagnostics.
    pub(super) display: String,
    /// Zero-based argument positions carrying policy values.
    pub(super) argument_positions: &'static [usize],
}

impl PolicyApi {
    /// Classifies a resolved call or method call against the curated catalogue.
    pub(super) fn for_expression(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Self> {
        // Prefer stable standard identities before consulting dependency-specific families.
        let identity = PolicyApiIdentity::resolve(cx, expression)?;
        let shape = Self::standard(&identity).or_else(|| Self::ecosystem(&identity))?;
        Some(Self::from_shape(&identity, shape))
    }

    /// Builds a diagnostic-facing API after validating its selected overload positions.
    fn from_shape(identity: &PolicyApiIdentity, shape: PolicyApiShape) -> Self {
        // Discard stale catalogue positions rather than indexing an unexpected overload.
        let positions_are_valid = shape
            .argument_positions
            .iter()
            .all(|position| *position < identity.argument_count);
        let argument_positions = if positions_are_valid {
            shape.argument_positions
        } else {
            &[]
        };

        // Retain a stable semantic path so the diagnostic explains its evidence.
        Self {
            category: shape.category,
            display: identity.path.clone(),
            argument_positions,
        }
    }

    /// Classifies standard duration, collection, pagination, and concurrency APIs.
    fn standard(identity: &PolicyApiIdentity) -> Option<PolicyApiShape> {
        // Exclude dependency APIs before consulting standard-library path families.
        // Nonstandard definitions belong to the ecosystem catalog instead.
        if !identity.is_standard() {
            return None;
        }

        // Duration constructors assign explicit units to otherwise raw numeric values.
        let item_name = identity.item_name.as_str();

        // A recognized duration constructor completes standard classification.
        if identity.path.contains("::time::Duration::")
            && POLICY_API_STANDARD_DURATION_CONSTRUCTOR_NAMES.contains(&item_name)
        {
            let argument_positions = if item_name == "new" {
                &[0, 1][..]
            } else {
                &[0][..]
            };

            return Some(PolicyApiShape {
                category: PolicyCategory::Timing,
                argument_positions,
            });
        }

        Self::standard_non_duration(identity)
    }

    /// Classifies standard collection, iterator, channel, and thread APIs.
    fn standard_non_duration(identity: &PolicyApiIdentity) -> Option<PolicyApiShape> {
        // Collection allocation methods encode resource-sizing policy.
        let item_name = identity.item_name.as_str();

        // Collection allocation bounds encode capacity policy directly.
        if identity.is_standard_collection()
            && POLICY_API_STANDARD_COLLECTION_CAPACITY_NAMES.contains(&item_name)
        {
            return Some(PolicyApiShape::single(PolicyCategory::Capacity));
        }

        // Retained length and iterator cardinality control output volume.
        // Truncation bounds the retained collection size.
        if identity.is_standard_collection() && item_name == "truncate" {
            return Some(PolicyApiShape::single(PolicyCategory::Truncation));
        }

        // Iterator take and skip operations encode pagination boundaries.
        if identity.path.contains("::Iterator::") && matches!(item_name, "take" | "skip") {
            return Some(PolicyApiShape::single(PolicyCategory::Pagination));
        }

        // Bounded channels constrain queued messages rather than active workers.
        // Synchronous channel construction fixes the queue capacity.
        if identity.path.contains("::sync::mpsc::") && item_name == "sync_channel" {
            return Some(PolicyApiShape::single(PolicyCategory::Capacity));
        }

        // Barriers define how many participants synchronize concurrently.
        let is_barrier = identity.path.contains("::sync::barrier::Barrier::")
            || identity.path.contains("::sync::Barrier::");

        // Barrier construction fixes the required concurrent participant count.
        if is_barrier && item_name == "new" {
            return Some(PolicyApiShape::single(PolicyCategory::Concurrency));
        }

        // Thread stack sizing is an explicit resource policy.
        let is_thread_builder = identity.path.contains("::thread::Builder::");
        (is_thread_builder && item_name == "stack_size")
            .then(|| PolicyApiShape::single(PolicyCategory::Capacity))
    }

    /// Routes dependency APIs to their intentionally small catalogue family.
    fn ecosystem(identity: &PolicyApiIdentity) -> Option<PolicyApiShape> {
        // Recognize the shared bounded-channel convention before crate-specific families.
        let crate_name = identity.crate_name.as_str();

        // The shared bounded constructor convention fixes channel capacity.
        if identity.item_name.as_str() == "bounded"
            && POLICY_API_ECOSYSTEM_BOUNDED_CHANNEL_CRATES.contains(&crate_name)
        {
            return Some(PolicyApiShape::single(PolicyCategory::Capacity));
        }

        // Select by defining crate so source aliases cannot forge semantic identity.
        match identity.crate_name.as_str() {
            "tokio" => Self::tokio(identity),
            "futures" | "futures_util" => Self::futures(identity),
            "rayon" | "rayon_core" => Self::rayon(identity),
            _ => None,
        }
    }

    /// Classifies Tokio semaphore, bounded-channel, and runtime sizing APIs.
    fn tokio(identity: &PolicyApiIdentity) -> Option<PolicyApiShape> {
        // Semaphore permits directly bound concurrent access to a shared resource.
        let item_name = identity.item_name.as_str();

        // Semaphore construction fixes the concurrent permit count.
        if identity.path.contains("::Semaphore::") && matches!(item_name, "new" | "const_new") {
            return Some(PolicyApiShape::single(PolicyCategory::Concurrency));
        }

        // Restrict channel matching to APIs whose first argument is actually capacity.
        if item_name == "channel"
            && (identity.path.contains("::sync::mpsc::")
                || identity.path.contains("::sync::broadcast::"))
        {
            return Some(PolicyApiShape::single(PolicyCategory::Capacity));
        }

        // Runtime builder values bound worker resources or scheduler cadence.
        if identity.path.contains("::runtime::Builder::")
            && POLICY_API_ECOSYSTEM_TOKIO_RUNTIME_NAMES.contains(&item_name)
        {
            return Some(PolicyApiShape::single(PolicyCategory::Concurrency));
        }

        // Uncatalogued tokio calls are intentionally ignored.
        None
    }

    /// Classifies Futures stream buffering and concurrent traversal APIs.
    fn futures(identity: &PolicyApiIdentity) -> Option<PolicyApiShape> {
        // Require the defining extension trait as well as a known concurrency method.
        let item_name = identity.item_name.as_str();
        let is_stream_extension =
            identity.path.contains("::StreamExt::") || identity.path.contains("::TryStreamExt::");

        // Calls outside recognized stream extensions or method names carry no concurrency bound.
        if !is_stream_extension
            || !POLICY_API_ECOSYSTEM_FUTURES_CONCURRENCY_NAMES.contains(&item_name)
        {
            return None;
        }

        // Every matched extension method uses its first argument as the bound.
        Some(PolicyApiShape::single(PolicyCategory::Concurrency))
    }

    /// Classifies Rayon thread-pool and indexed scheduling limits.
    fn rayon(identity: &PolicyApiIdentity) -> Option<PolicyApiShape> {
        // Thread-pool builder values directly control parallel resource use.
        let item_name = identity.item_name.as_str();

        // Thread count and stack sizing bound the pool's resource use.
        if identity.path.contains("::ThreadPoolBuilder::")
            && matches!(item_name, "num_threads" | "stack_size")
        {
            return Some(PolicyApiShape::single(PolicyCategory::Concurrency));
        }

        // Indexed iterator limits tune how much work one scheduling unit receives.
        // Minimum and maximum chunk lengths encode scheduling capacity.
        if identity.path.contains("::IndexedParallelIterator::")
            && matches!(item_name, "with_min_len" | "with_max_len")
        {
            return Some(PolicyApiShape::single(PolicyCategory::Capacity));
        }
        None
    }
}
