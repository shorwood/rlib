extern crate rustc_hir;
extern crate rustc_span;

use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};

use rustc_hir::def_id::LocalDefId;
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
        let index = self.next_index;
        self.next_index += 1;
        self.indices[vertex] = Some(index);
        self.lowlinks[vertex] = index;
        self.stack.push(vertex);
        self.on_stack[vertex] = true;

        for neighbour in &self.edges[vertex] {
            if self.indices[*neighbour].is_none() {
                self.visit(*neighbour);
                self.lowlinks[vertex] = self.lowlinks[vertex].min(self.lowlinks[*neighbour]);
            } else if self.on_stack[*neighbour] {
                self.lowlinks[vertex] = self.lowlinks[vertex]
                    .min(self.indices[*neighbour].expect("visited stack member has an index"));
            }
        }

        if self.lowlinks[vertex] == index {
            let mut component = Vec::new();
            loop {
                let member = self
                    .stack
                    .pop()
                    .expect("root is present on its Tarjan stack");
                self.on_stack[member] = false;
                component.push(member);
                if member == vertex {
                    break;
                }
            }
            self.components.push(component);
        }
    }

    fn run(mut self) -> Vec<Vec<usize>> {
        for vertex in 0..self.edges.len() {
            if self.indices[vertex].is_none() {
                self.visit(vertex);
            }
        }
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

    /// Returns the stable dependency-first permutation for these declarations.
    ///
    /// Strongly connected components are collapsed before sorting, so recursive declarations stay
    /// contiguous and retain their source order. Category and visibility only break ties between
    /// components whose dependencies have already been emitted.
    pub(crate) fn declaration_order(&self) -> Vec<usize> {
        let owner = self
            .iter()
            .enumerate()
            .flat_map(|(index, node)| node.defs.iter().map(move |def| (*def, index)))
            .collect::<HashMap<_, _>>();
        let edges = self
            .iter()
            .enumerate()
            .map(|(index, node)| {
                node.dependencies
                    .iter()
                    .filter_map(|dependency| owner.get(dependency).copied())
                    .filter(|dependency| *dependency != index)
                    .collect::<HashSet<_>>()
            })
            .collect::<Vec<_>>();

        let components = Tarjan::new(&edges).run();
        let mut component_of = vec![0; self.len()];
        for (component, members) in components.iter().enumerate() {
            for member in members {
                component_of[*member] = component;
            }
        }

        let mut dependencies = vec![HashSet::new(); components.len()];
        for (node, node_edges) in edges.iter().enumerate() {
            for dependency in node_edges {
                let from = component_of[node];
                let to = component_of[*dependency];
                if from != to {
                    dependencies[from].insert(to);
                }
            }
        }

        let mut emitted = vec![false; components.len()];
        let mut order = Vec::with_capacity(self.len());
        while order.len() < self.len() {
            let next = (0..components.len())
                .filter(|component| {
                    !emitted[*component]
                        && dependencies[*component]
                            .iter()
                            .all(|dependency| emitted[*dependency])
                })
                .min_by_key(|component| {
                    let first = components[*component][0];
                    (self[first].category, !self[first].is_outward_visible, first)
                });
            let Some(next) = next else { break };
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
