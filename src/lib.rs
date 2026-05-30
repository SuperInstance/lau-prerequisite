//! # lau-prerequisite
//!
//! Models the prerequisite/dependency graph of PLATO concepts — what must be
//! learned before what.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Unique identifier for a concept.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConceptId(pub String);

impl ConceptId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConceptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ConceptId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Category of a PLATO concept.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ConceptCategory {
    Foundation,
    Conservation,
    Prediction,
    Topology,
    Composition,
    Systems,
}

/// A single PLATO concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub id: ConceptId,
    pub name: String,
    pub kid_name: String,
    pub description: String,
    pub difficulty: u32,
    pub category: ConceptCategory,
}

impl Concept {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kid_name: impl Into<String>,
        description: impl Into<String>,
        difficulty: u32,
        category: ConceptCategory,
    ) -> Self {
        Self {
            id: ConceptId::new(id),
            name: name.into(),
            kid_name: kid_name.into(),
            description: description.into(),
            difficulty: difficulty.clamp(1, 5),
            category,
        }
    }
}

/// Directed prerequisite graph: edges go from prerequisite → dependent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrereqGraph {
    pub concepts: HashMap<ConceptId, Concept>,
    pub edges: Vec<(ConceptId, ConceptId)>,
}

impl PrereqGraph {
    pub fn new() -> Self {
        Self {
            concepts: HashMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_concept(&mut self, concept: Concept) {
        self.concepts.insert(concept.id.clone(), concept);
    }

    pub fn add_prereq(&mut self, from: &str, to: &str) {
        let from_id = ConceptId::new(from);
        let to_id = ConceptId::new(to);
        // Avoid duplicate edges
        if !self.edges.contains(&(from_id.clone(), to_id.clone())) {
            self.edges.push((from_id, to_id));
        }
    }

    /// Direct prerequisites of a concept.
    pub fn prerequisites_of(&self, concept: &ConceptId) -> Vec<&Concept> {
        self.edges
            .iter()
            .filter(|(_, to)| to == concept)
            .filter_map(|(from, _)| self.concepts.get(from))
            .collect()
    }

    /// Transitive closure of prerequisites (recursive).
    pub fn all_prerequisites(&self, concept: &ConceptId) -> Vec<&Concept> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        self.collect_all_prereqs(concept, &mut visited, &mut result);
        result
    }

    fn collect_all_prereqs<'a>(
        &'a self,
        concept: &ConceptId,
        visited: &mut HashSet<ConceptId>,
        result: &mut Vec<&'a Concept>,
    ) {
        for prereq in self.prerequisites_of(concept) {
            if visited.insert(prereq.id.clone()) {
                result.push(prereq);
                self.collect_all_prereqs(&prereq.id, visited, result);
            }
        }
    }

    /// Concepts that depend on this one (what unlocks after learning this).
    pub fn dependents(&self, concept: &ConceptId) -> Vec<&Concept> {
        self.edges
            .iter()
            .filter(|(from, _)| from == concept)
            .filter_map(|(_, to)| self.concepts.get(to))
            .collect()
    }

    /// Check if all direct prerequisites of `concept` are in `learned`.
    pub fn is_unlocked(&self, concept: &ConceptId, learned: &[ConceptId]) -> bool {
        let learned_set: HashSet<&ConceptId> = learned.iter().collect();
        self.prerequisites_of(concept)
            .iter()
            .all(|p| learned_set.contains(&p.id))
    }

    /// What can I learn next, given what I've already learned?
    pub fn next_unlockable(&self, learned: &[ConceptId]) -> Vec<&Concept> {
        let learned_set: HashSet<&ConceptId> = learned.iter().collect();
        self.concepts
            .values()
            .filter(|c| !learned_set.contains(&c.id))
            .filter(|c| self.is_unlocked(&c.id, learned))
            .collect()
    }

    /// Topological sort — valid learning order.
    pub fn topological_sort(&self) -> Vec<ConceptId> {
        let mut in_degree: HashMap<&ConceptId, usize> = self
            .concepts
            .keys()
            .map(|id| (id, 0))
            .collect();

        for (_, to) in &self.edges {
            if let Some(deg) = in_degree.get_mut(to) {
                *deg += 1;
            }
        }

        let mut queue: VecDeque<&ConceptId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        // Build adjacency list for reverse lookup
        let mut adj: HashMap<&ConceptId, Vec<&ConceptId>> = HashMap::new();
        for (from, to) in &self.edges {
            adj.entry(from).or_default().push(to);
        }

        let mut result = Vec::new();
        while let Some(id) = queue.pop_front() {
            result.push(id.clone());
            if let Some(neighbors) = adj.get(id) {
                for &neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        result
    }

    /// Shortest learning path between two concepts (BFS).
    pub fn learning_path(&self, from: &ConceptId, to: &ConceptId) -> Vec<ConceptId> {
        if from == to {
            return vec![from.clone()];
        }

        // BFS on prerequisite edges (reverse direction: from dependent to prereqs)
        // Actually we want a path that respects the prerequisite order,
        // so we traverse from `from` forward through dependents to reach `to`.
        let mut adj: HashMap<&ConceptId, Vec<&ConceptId>> = HashMap::new();
        for (src, dst) in &self.edges {
            adj.entry(src).or_default().push(dst);
        }

        let mut queue = VecDeque::new();
        let mut parent: HashMap<&ConceptId, &ConceptId> = HashMap::new();
        let mut visited = HashSet::new();

        queue.push_back(from);
        visited.insert(from);

        while let Some(current) = queue.pop_front() {
            if current == to {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node: &ConceptId = to;
                path.push(node.clone());
                while let Some(&p) = parent.get(node) {
                    path.push(p.clone());
                    node = p;
                }
                path.reverse();
                return path;
            }

            if let Some(neighbors) = adj.get(current) {
                for &neighbor in neighbors {
                    if visited.insert(neighbor) {
                        parent.insert(neighbor, current);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        Vec::new() // no path found
    }

    /// Build the pre-built PLATO concept graph.
    pub fn plato_graph() -> Self {
        let mut g = Self::new();

        // Foundation chain
        g.add_concept(Concept::new(
            "vibe", "Vibe", "Vibe",
            "The fundamental sense of a system's state — the felt quality of its behavior.",
            1, ConceptCategory::Foundation,
        ));
        g.add_concept(Concept::new(
            "deadband", "Deadband", "Deadband",
            "The zone of insensitivity where small changes produce no output — learning to ignore noise.",
            1, ConceptCategory::Foundation,
        ));
        g.add_concept(Concept::new(
            "signal", "Signal", "Signal",
            "Distinguishing meaningful change from noise; what rises above the deadband.",
            2, ConceptCategory::Foundation,
        ));
        g.add_concept(Concept::new(
            "murmur", "Murmur", "Murmur",
            "The faint but persistent pattern beneath noise — coherent whispers of structure.",
            2, ConceptCategory::Foundation,
        ));

        // Conservation chain
        g.add_concept(Concept::new(
            "conservation", "Conservation", "Conservation",
            "What is preserved across transformations — the invariants that define identity.",
            2, ConceptCategory::Conservation,
        ));
        g.add_concept(Concept::new(
            "distillation", "Distillation", "Distillation",
            "Extracting the essential structure from a richer representation while conserving meaning.",
            3, ConceptCategory::Conservation,
        ));
        g.add_concept(Concept::new(
            "fibonacci", "Fibonacci", "Fibonacci",
            "Self-similar growth patterns that conserve ratio across scales — nature's recursion.",
            3, ConceptCategory::Conservation,
        ));
        g.add_concept(Concept::new(
            "topology", "Topology", "Topology",
            "The shape that persists through continuous deformation — the ultimate conservation law.",
            4, ConceptCategory::Topology,
        ));

        // Prediction chain
        g.add_concept(Concept::new(
            "jepa", "JEPA", "JEPA",
            "Joint-Embedding Predictive Architecture — predicting representations, not pixels.",
            2, ConceptCategory::Prediction,
        ));
        g.add_concept(Concept::new(
            "room_lifecycle", "Room Lifecycle", "Room Lifecycle",
            "How spaces evolve through use — predicting the arc of habitation.",
            3, ConceptCategory::Prediction,
        ));
        g.add_concept(Concept::new(
            "dissolution", "Dissolution", "Dissolution",
            "The predicted endpoint when structure yields to entropy — graceful decay of form.",
            4, ConceptCategory::Prediction,
        ));

        // Composition chain
        g.add_concept(Concept::new(
            "category_theory", "Category Theory", "Category Theory",
            "The mathematics of composability — objects, morphisms, and universal patterns.",
            3, ConceptCategory::Composition,
        ));
        g.add_concept(Concept::new(
            "agent_composition", "Agent Composition", "Agent Composition",
            "Composing intelligent agents from simpler primitives via categorical constructs.",
            4, ConceptCategory::Composition,
        ));
        g.add_concept(Concept::new(
            "functor", "Functor", "Functor",
            "Structure-preserving maps between categories — the bridge between worlds.",
            4, ConceptCategory::Composition,
        ));

        // Systems
        g.add_concept(Concept::new(
            "grand_pattern", "Grand Pattern", "Grand Pattern",
            "The unified architecture underlying all PLATO concepts — the system of systems.",
            5, ConceptCategory::Systems,
        ));

        // Foundation edges: Vibe → Deadband → Signal → Murmur
        g.add_prereq("vibe", "deadband");
        g.add_prereq("deadband", "signal");
        g.add_prereq("signal", "murmur");

        // Conservation edges: Vibe → Conservation → Distillation → Fibonacci → Topology
        g.add_prereq("vibe", "conservation");
        g.add_prereq("conservation", "distillation");
        g.add_prereq("distillation", "fibonacci");
        g.add_prereq("fibonacci", "topology");

        // Prediction edges: Vibe → JEPA → RoomLifecycle → Dissolution
        g.add_prereq("vibe", "jepa");
        g.add_prereq("jepa", "room_lifecycle");
        g.add_prereq("room_lifecycle", "dissolution");

        // Composition edges: Conservation → CategoryTheory → AgentComposition → Functor
        g.add_prereq("conservation", "category_theory");
        g.add_prereq("category_theory", "agent_composition");
        g.add_prereq("agent_composition", "functor");

        // Systems edges: All of the above → GrandPattern
        g.add_prereq("murmur", "grand_pattern");
        g.add_prereq("topology", "grand_pattern");
        g.add_prereq("dissolution", "grand_pattern");
        g.add_prereq("functor", "grand_pattern");

        g
    }
}

impl Default for PrereqGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// A computed learning path from current knowledge to a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningPath {
    pub steps: Vec<ConceptId>,
    pub total_difficulty: u32,
    pub estimated_ticks: u64,
}

impl LearningPath {
    /// Compute a learning path from `learned` to `target` in the given graph.
    /// Uses topological ordering to find the shortest valid path.
    pub fn from_graph(
        graph: &PrereqGraph,
        learned: &[ConceptId],
        target: &ConceptId,
    ) -> Option<Self> {
        if !graph.concepts.contains_key(target) {
            return None;
        }

        let learned_set: HashSet<&ConceptId> = learned.iter().collect();
        if learned_set.contains(target) {
            return Some(Self {
                steps: Vec::new(),
                total_difficulty: 0,
                estimated_ticks: 0,
            });
        }

        // BFS backwards from target to find what we need
        let mut needed = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(target);
        visited.insert(target.clone());

        while let Some(concept) = queue.pop_front() {
            if learned_set.contains(concept) {
                continue;
            }
            needed.push(concept.clone());
            for prereq in graph.prerequisites_of(concept) {
                if visited.insert(prereq.id.clone()) {
                    queue.push_back(&prereq.id);
                }
            }
        }

        if needed.is_empty() {
            return Some(Self {
                steps: Vec::new(),
                total_difficulty: 0,
                estimated_ticks: 0,
            });
        }

        // Sort needed concepts topologically
        let topo = graph.topological_sort();
        let needed_set: HashSet<&ConceptId> = needed.iter().collect();
        let steps: Vec<ConceptId> = topo
            .into_iter()
            .filter(|id| needed_set.contains(id))
            .collect();

        let total_difficulty: u32 = steps
            .iter()
            .filter_map(|id| graph.concepts.get(id))
            .map(|c| c.difficulty)
            .sum();

        // Estimated ticks: sum of difficulty * 10 per step
        let estimated_ticks = total_difficulty as u64 * 10;

        Some(Self {
            steps,
            total_difficulty,
            estimated_ticks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_graph() -> PrereqGraph {
        let mut g = PrereqGraph::new();
        g.add_concept(Concept::new("a", "A", "A", "first", 1, ConceptCategory::Foundation));
        g.add_concept(Concept::new("b", "B", "B", "second", 2, ConceptCategory::Foundation));
        g.add_concept(Concept::new("c", "C", "C", "third", 3, ConceptCategory::Conservation));
        g.add_concept(Concept::new("d", "D", "D", "fourth", 4, ConceptCategory::Systems));
        g.add_prereq("a", "b");
        g.add_prereq("b", "c");
        g.add_prereq("c", "d");
        g
    }

    #[test]
    fn test_add_concept() {
        let g = test_graph();
        assert!(g.concepts.contains_key(&ConceptId::new("a")));
        assert_eq!(g.concepts.len(), 4);
    }

    #[test]
    fn test_add_prereq_no_duplicates() {
        let mut g = test_graph();
        g.add_prereq("a", "b");
        g.add_prereq("a", "b");
        assert_eq!(g.edges.len(), 3);
    }

    #[test]
    fn test_prerequisites_of() {
        let g = test_graph();
        let prereqs = g.prerequisites_of(&ConceptId::new("c"));
        assert_eq!(prereqs.len(), 1);
        assert_eq!(prereqs[0].id, ConceptId::new("b"));
    }

    #[test]
    fn test_prerequisites_of_root() {
        let g = test_graph();
        let prereqs = g.prerequisites_of(&ConceptId::new("a"));
        assert!(prereqs.is_empty());
    }

    #[test]
    fn test_all_prerequisites() {
        let g = test_graph();
        let all = g.all_prerequisites(&ConceptId::new("d"));
        let ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"c"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"a"));
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_dependents() {
        let g = test_graph();
        let deps = g.dependents(&ConceptId::new("a"));
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].id, ConceptId::new("b"));
    }

    #[test]
    fn test_dependents_leaf() {
        let g = test_graph();
        let deps = g.dependents(&ConceptId::new("d"));
        assert!(deps.is_empty());
    }

    #[test]
    fn test_is_unlocked() {
        let g = test_graph();
        assert!(g.is_unlocked(&ConceptId::new("a"), &[]));
        assert!(!g.is_unlocked(&ConceptId::new("b"), &[]));
        assert!(g.is_unlocked(&ConceptId::new("b"), &[ConceptId::new("a")]));
    }

    #[test]
    fn test_next_unlockable_empty() {
        let g = test_graph();
        let next = g.next_unlockable(&[]);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].id, ConceptId::new("a"));
    }

    #[test]
    fn test_next_unlockable_partial() {
        let g = test_graph();
        let next = g.next_unlockable(&[ConceptId::new("a"), ConceptId::new("b")]);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].id, ConceptId::new("c"));
    }

    #[test]
    fn test_next_unlockable_all_learned() {
        let g = test_graph();
        let learned = vec![
            ConceptId::new("a"),
            ConceptId::new("b"),
            ConceptId::new("c"),
            ConceptId::new("d"),
        ];
        let next = g.next_unlockable(&learned);
        assert!(next.is_empty());
    }

    #[test]
    fn test_topological_sort() {
        let g = test_graph();
        let topo = g.topological_sort();
        assert_eq!(topo.len(), 4);
        let pos_a = topo.iter().position(|x| x == &ConceptId::new("a")).unwrap();
        let pos_b = topo.iter().position(|x| x == &ConceptId::new("b")).unwrap();
        let pos_c = topo.iter().position(|x| x == &ConceptId::new("c")).unwrap();
        let pos_d = topo.iter().position(|x| x == &ConceptId::new("d")).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
        assert!(pos_c < pos_d);
    }

    #[test]
    fn test_learning_path_direct() {
        let g = test_graph();
        let path = g.learning_path(&ConceptId::new("a"), &ConceptId::new("c"));
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], ConceptId::new("a"));
        assert_eq!(path[2], ConceptId::new("c"));
    }

    #[test]
    fn test_learning_path_same_node() {
        let g = test_graph();
        let path = g.learning_path(&ConceptId::new("a"), &ConceptId::new("a"));
        assert_eq!(path, vec![ConceptId::new("a")]);
    }

    #[test]
    fn test_learning_path_no_path() {
        let g = test_graph();
        // d has no outgoing edges to a
        let path = g.learning_path(&ConceptId::new("d"), &ConceptId::new("a"));
        assert!(path.is_empty());
    }

    #[test]
    fn test_plato_graph_concepts() {
        let g = PrereqGraph::plato_graph();
        assert_eq!(g.concepts.len(), 15);
        assert!(g.concepts.contains_key(&ConceptId::new("vibe")));
        assert!(g.concepts.contains_key(&ConceptId::new("grand_pattern")));
    }

    #[test]
    fn test_plato_graph_edges() {
        let g = PrereqGraph::plato_graph();
        assert!(g.edges.contains(&(ConceptId::new("vibe"), ConceptId::new("deadband"))));
        assert!(g.edges.contains(&(ConceptId::new("murmur"), ConceptId::new("grand_pattern"))));
    }

    #[test]
    fn test_plato_topological_sort() {
        let g = PrereqGraph::plato_graph();
        let topo = g.topological_sort();
        assert_eq!(topo.len(), 15);
        let pos_vibe = topo.iter().position(|x| x.0 == "vibe").unwrap();
        let pos_gp = topo.iter().position(|x| x.0 == "grand_pattern").unwrap();
        assert!(pos_vibe < pos_gp);
    }

    #[test]
    fn test_plato_next_unlockable() {
        let g = PrereqGraph::plato_graph();
        let next = g.next_unlockable(&[]);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].id, ConceptId::new("vibe"));
    }

    #[test]
    fn test_plato_all_prereqs_of_grand_pattern() {
        let g = PrereqGraph::plato_graph();
        let all = g.all_prerequisites(&ConceptId::new("grand_pattern"));
        assert_eq!(all.len(), 14); // everything else
    }

    #[test]
    fn test_plato_dependents_of_vibe() {
        let g = PrereqGraph::plato_graph();
        let deps = g.dependents(&ConceptId::new("vibe"));
        assert_eq!(deps.len(), 3); // deadband, conservation, jepa
    }

    #[test]
    fn test_learning_path_from_graph() {
        let g = PrereqGraph::plato_graph();
        let path = LearningPath::from_graph(&g, &[], &ConceptId::new("signal")).unwrap();
        assert_eq!(path.steps.len(), 3); // vibe, deadband, signal
        assert!(path.total_difficulty > 0);
        assert!(path.estimated_ticks > 0);
    }

    #[test]
    fn test_learning_path_from_graph_already_learned() {
        let g = PrereqGraph::plato_graph();
        let path = LearningPath::from_graph(
            &g,
            &[ConceptId::new("vibe")],
            &ConceptId::new("vibe"),
        ).unwrap();
        assert!(path.steps.is_empty());
    }

    #[test]
    fn test_learning_path_from_graph_unknown() {
        let g = PrereqGraph::plato_graph();
        assert!(LearningPath::from_graph(&g, &[], &ConceptId::new("nonexistent")).is_none());
    }

    #[test]
    fn test_learning_path_plato_to_grand_pattern() {
        let g = PrereqGraph::plato_graph();
        let path = LearningPath::from_graph(&g, &[], &ConceptId::new("grand_pattern")).unwrap();
        assert_eq!(path.steps.len(), 15); // all concepts needed
        // grand_pattern should be last
        assert_eq!(path.steps.last(), Some(&ConceptId::new("grand_pattern")));
    }

    #[test]
    fn test_serde_roundtrip_concept() {
        let c = Concept::new("test", "Test", "T", "A test concept", 3, ConceptCategory::Prediction);
        let json = serde_json::to_string(&c).unwrap();
        let c2: Concept = serde_json::from_str(&json).unwrap();
        assert_eq!(c2.id, c.id);
        assert_eq!(c2.name, c.name);
        assert_eq!(c2.difficulty, c.difficulty);
    }

    #[test]
    fn test_serde_roundtrip_graph() {
        let g = PrereqGraph::plato_graph();
        let json = serde_json::to_string(&g).unwrap();
        let g2: PrereqGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(g2.concepts.len(), g.concepts.len());
        assert_eq!(g2.edges.len(), g.edges.len());
    }

    #[test]
    fn test_difficulty_clamped() {
        let c = Concept::new("x", "X", "X", "desc", 10, ConceptCategory::Foundation);
        assert_eq!(c.difficulty, 5);
        let c2 = Concept::new("y", "Y", "Y", "desc", 0, ConceptCategory::Foundation);
        assert_eq!(c2.difficulty, 1);
    }
}
