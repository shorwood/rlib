extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use rustc_hir::def_id::LocalDefId;
use rustc_lint::LateContext;
use rustc_span::Span;

// -----------------------------------------------------------------------------
// Tarjan: Strongly connected component traversal
// -----------------------------------------------------------------------------

/// Finds strongly connected components in a declaration dependency graph.
struct Tarjan<'graph> {
    edges: &'graph [HashSet<usize>],
    next_index: usize,
    indices: Vec<Option<usize>>,
    lowlinks: Vec<usize>,
    stack: Vec<usize>,
    on_stack: Vec<bool>,
    components: Vec<Vec<usize>>,
}

impl<'graph> Tarjan<'graph> {
    fn new(edges: &'graph [HashSet<usize>]) -> Self {
        Self {
            edges,
            next_index: 0,
            indices: vec![None; edges.len()],
            lowlinks: vec![0; edges.len()],
            stack: Vec::new(),
            on_stack: vec![false; edges.len()],
            components: Vec::new(),
        }
    }

    fn visit(&mut self, vertex: usize) {
        // Mark the vertex as active in this depth-first traversal.
        let index = self.next_index;
        self.next_index += 1;
        self.indices[vertex] = Some(index);
        self.lowlinks[vertex] = index;
        self.stack.push(vertex);
        self.on_stack[vertex] = true;

        // Propagate the earliest reachable index through every outgoing edge.
        for neighbour in &self.edges[vertex] {
            if self.indices[*neighbour].is_none() {
                self.visit(*neighbour);
                self.lowlinks[vertex] = self.lowlinks[vertex].min(self.lowlinks[*neighbour]);
            } else if self.on_stack[*neighbour] {
                self.lowlinks[vertex] = self.lowlinks[vertex]
                    .min(self.indices[*neighbour].expect("visited stack member has an index"));
            }
        }

        if self.lowlinks[vertex] != index {
            return;
        }

        // Pop the complete strongly connected component rooted at this vertex.
        let mut component = Vec::new();
        loop {
            let member = self
                .stack
                .pop()
                .expect("root is present on its Tarjan stack");
            self.on_stack[member] = false;
            component.push(member);
            if member != vertex {
                continue;
            }
            break;
        }
        self.components.push(component);
    }

    fn run(mut self) -> Vec<Vec<usize>> {
        for vertex in 0..self.edges.len() {
            if self.indices[vertex].is_some() {
                continue;
            }
            self.visit(vertex);
        }

        // Stabilize members and components in their original source order.
        for component in &mut self.components {
            component.sort_unstable();
        }
        self.components.sort_by_key(|component| component[0]);
        self.components
    }
}

// -----------------------------------------------------------------------------
// DeclarationNode: Dependency graph declarations
// -----------------------------------------------------------------------------

/// A movable declaration or an inseparable declaration group.
pub(crate) struct DeclarationNode {
    pub(crate) defs: Vec<LocalDefId>,
    pub(crate) name: String,
    pub(crate) span: Span,
    /// Zero for unsectioned declarations, otherwise the authored section's source ordinal.
    pub(crate) section: usize,
    pub(crate) category: u8,
    pub(crate) is_outward_visible: bool,
    pub(crate) dependencies: HashSet<LocalDefId>,
}

/// A collection of declarations that can compute its stable dependency-first order.
pub(crate) struct DeclarationNodeList {
    items: Vec<DeclarationNode>,
}

impl DeclarationNodeList {
    pub(crate) fn new(items: Vec<DeclarationNode>) -> Self {
        Self { items }
    }

    /// Returns the first source position whose expected declaration differs.
    pub(crate) fn first_mismatch(source: &[usize], ordering: &[usize]) -> usize {
        let mut pairs = source.iter().zip(ordering);
        pairs
            .position(|(actual, expected)| actual != expected)
            .expect("different orders have a mismatch")
    }

    fn add_component_dependency(
        dependencies: &mut [HashSet<usize>],
        component_of: &[usize],
        node: usize,
        dependency: usize,
    ) {
        let from = component_of[node];
        let to = component_of[dependency];
        if from == to {
            return;
        }
        dependencies[from].insert(to);
    }

    /// Formats an ordering as a readable sequence of declaration names.
    pub(crate) fn formatted_names(&self, ordering: &[usize]) -> String {
        let names = ordering
            .iter()
            .map(|index| format!("`{}`", self[*index].name));
        names.collect::<Vec<_>>().join(", ")
    }

    /// Returns whether every grouped declaration can participate in an atomic source edit.
    pub(crate) fn has_only_unattributed_definitions(&self, cx: &LateContext<'_>) -> bool {
        self.iter().all(|node| {
            node.defs.iter().all(|definition| {
                cx.tcx
                    .hir_attrs(rustc_hir::HirId::make_owner(*definition))
                    .is_empty()
            })
        })
    }

    fn node_dependencies(
        &self,
        index: usize,
        node: &DeclarationNode,
        owner: &HashMap<LocalDefId, usize>,
    ) -> HashSet<usize> {
        let resolved = node
            .dependencies
            .iter()
            .filter_map(|dependency| owner.get(dependency).copied());

        // Keep only meaningful edges within the declaration's authored section.
        resolved
            .filter(|dependency| {
                *dependency != index && self[*dependency].section == self[index].section
            })
            .collect()
    }

    /// Returns the stable dependency-first permutation for these declarations.
    ///
    /// Strongly connected components are collapsed before sorting, so recursive declarations stay
    /// contiguous and retain their source order. Category and visibility only break ties between
    /// components whose dependencies have already been emitted.
    pub(crate) fn declaration_order(&self) -> Vec<usize> {
        // Map every declaration identity back to its inseparable source node.
        let indexed = self.iter().enumerate();
        let owners =
            indexed.flat_map(|(index, node)| node.defs.iter().map(move |def| (*def, index)));
        let owner = owners.collect::<HashMap<_, _>>();

        // Build dependency edges that remain within each authored section.
        let indexed = self.iter().enumerate();
        let node_edges = indexed.map(|(index, node)| self.node_dependencies(index, node, &owner));
        let edges = node_edges.collect::<Vec<_>>();

        // Collapse recursive components before applying stable tie breakers.
        let components = Tarjan::new(&edges).run();
        let mut component_of = vec![0; self.len()];
        for (component, members) in components.iter().enumerate() {
            for member in members {
                component_of[*member] = component;
            }
        }

        // Lift node dependencies onto the collapsed component graph.
        let mut dependencies = vec![HashSet::new(); components.len()];
        for (node, node_edges) in edges.iter().enumerate() {
            for dependency in node_edges {
                Self::add_component_dependency(&mut dependencies, &component_of, node, *dependency);
            }
        }

        // Repeatedly select the earliest ready component under the stable policy.
        let mut emitted = vec![false; components.len()];
        let mut order = Vec::with_capacity(self.len());
        while order.len() < self.len() {
            // Select the earliest component whose dependencies have all been emitted.
            let next = (0..components.len())
                .filter(|component| {
                    !emitted[*component]
                        && dependencies[*component]
                            .iter()
                            .all(|dependency| emitted[*dependency])
                })
                .min_by_key(|component| {
                    let first = components[*component][0];
                    (
                        self[first].section,
                        self[first].category,
                        !self[first].is_outward_visible,
                        first,
                    )
                });

            // Stop once a malformed graph offers no remaining ready component.
            let Some(next) = next else { break };

            // Append the complete recursive component without disturbing source order.
            emitted[next] = true;
            order.extend(components[next].iter().copied());
        }
        order
    }
}

impl Deref for DeclarationNodeList {
    type Target = [DeclarationNode];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}
