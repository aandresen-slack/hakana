use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::{
    data_flow::graph::{DataFlowGraph, GraphKind},
    issue::Issue,
    symbol_references::SymbolReferences,
};

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub emitted_issues: BTreeMap<String, Vec<Issue>>,
    pub replacements: HashMap<String, BTreeMap<(usize, usize), String>>,
    pub mixed_source_counts: HashMap<String, HashSet<String>>,
    pub taint_flow_graph: DataFlowGraph,
    pub symbol_references: SymbolReferences,
}

impl AnalysisResult {
    pub fn new() -> Self {
        Self {
            emitted_issues: BTreeMap::new(),
            replacements: HashMap::new(),
            mixed_source_counts: HashMap::new(),
            taint_flow_graph: DataFlowGraph::new(GraphKind::Taint),
            symbol_references: SymbolReferences::new(),
        }
    }

    pub fn extend(&mut self, other: Self) {
        self.emitted_issues.extend(other.emitted_issues);
        self.replacements.extend(other.replacements);
        for (id, c) in other.mixed_source_counts {
            self.mixed_source_counts
                .entry(id)
                .or_insert_with(HashSet::new)
                .extend(c);
        }
        self.taint_flow_graph.add_graph(other.taint_flow_graph);
        self.symbol_references.extend(other.symbol_references);
    }
}

#[derive(Serialize)]
pub struct CheckPointEntry {
    pub case: String,
    pub level: String,
    pub filename: String,
    pub line: usize,
    pub output: String,
}

impl CheckPointEntry {
    pub fn from_issue(issue: &Issue) -> Self {
        Self {
            output: issue.description.clone(),
            level: "failure".to_string(),
            filename: (*issue.pos.file_path).clone(),
            line: issue.pos.start_line,
            case: issue.kind.to_string(),
        }
    }
}
