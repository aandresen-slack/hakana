use std::{collections::BTreeMap, time::Duration};

use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;

use crate::{
    code_location::FilePath,
    data_flow::graph::{DataFlowGraph, GraphKind},
    issue::{Issue, IssueKind},
    symbol_references::SymbolReferences,
    Interner,
};

#[derive(Clone, Debug)]
pub enum Replacement {
    Remove,
    TrimPrecedingWhitespace(u32),
    TrimTrailingWhitespace(u32),
    Substitute(String),
}

#[derive(Clone, Debug)]
pub struct AnalysisResult {
    pub emitted_issues: FxHashMap<FilePath, Vec<Issue>>,
    pub emitted_definition_issues: FxHashMap<FilePath, Vec<Issue>>,
    pub replacements: FxHashMap<FilePath, BTreeMap<(u32, u32), Replacement>>,
    pub insertions: FxHashMap<FilePath, BTreeMap<u32, Vec<String>>>,
    pub mixed_source_counts: FxHashMap<String, FxHashSet<String>>,
    pub program_dataflow_graph: DataFlowGraph,
    pub symbol_references: SymbolReferences,
    pub issue_counts: FxHashMap<IssueKind, usize>,
    pub time_in_analysis: Duration,
}

impl AnalysisResult {
    pub fn new(
        program_dataflow_graph_kind: GraphKind,
        symbol_references: SymbolReferences,
    ) -> Self {
        Self {
            emitted_issues: FxHashMap::default(),
            emitted_definition_issues: FxHashMap::default(),
            replacements: FxHashMap::default(),
            insertions: FxHashMap::default(),
            mixed_source_counts: FxHashMap::default(),
            program_dataflow_graph: DataFlowGraph::new(program_dataflow_graph_kind),
            issue_counts: FxHashMap::default(),
            symbol_references,
            time_in_analysis: Duration::default(),
        }
    }

    pub fn extend(&mut self, other: Self) {
        for (file_path, issues) in other.emitted_issues {
            self.emitted_issues
                .entry(file_path)
                .or_default()
                .extend(issues);
        }
        self.replacements.extend(other.replacements);
        self.insertions.extend(other.insertions);
        for (id, c) in other.mixed_source_counts {
            self.mixed_source_counts.entry(id).or_default().extend(c);
        }
        self.program_dataflow_graph
            .add_graph(other.program_dataflow_graph);
        self.symbol_references.extend(other.symbol_references);
        for (kind, count) in other.issue_counts {
            *self.issue_counts.entry(kind).or_insert(0) += count;
        }
    }

    pub fn get_all_issues(
        &self,
        interner: &Interner,
        root_dir: &str,
        use_relative_path: bool,
    ) -> BTreeMap<String, Vec<&Issue>> {
        let mut issues = self
            .emitted_issues
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| {
                (
                    if use_relative_path {
                        k.get_relative_path(interner, root_dir)
                    } else {
                        interner.lookup(&k.0).to_string()
                    },
                    {
                        let mut file_issues = v.iter().collect::<Vec<_>>();
                        file_issues.sort_by(|a, b| a.pos.start_offset.cmp(&b.pos.start_offset));
                        file_issues
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        for (file_path, file_definition_issues) in &self.emitted_definition_issues {
            let file_path = if use_relative_path {
                file_path.get_relative_path(interner, root_dir)
            } else {
                interner.lookup(&file_path.0).to_string()
            };

            if let Some(file_issues) = issues.get_mut(&file_path) {
                file_issues.extend(file_definition_issues);
                file_issues.sort_by(|a, b| a.pos.start_offset.cmp(&b.pos.start_offset));
            } else {
                let mut file_issues: Vec<_> = file_definition_issues.iter().collect::<Vec<_>>();
                file_issues.sort_by(|a, b| a.pos.start_offset.cmp(&b.pos.start_offset));
                issues.insert(file_path, file_issues);
            }
        }

        issues
    }
}

#[derive(Serialize)]
pub struct CheckPointEntry {
    pub case: String,
    pub level: String,
    pub filename: String,
    pub line: u32,
    pub output: String,
}

impl CheckPointEntry {
    pub fn from_issue(issue: &Issue, path: &str) -> Self {
        Self {
            output: issue.description.clone(),
            level: "failure".to_string(),
            filename: path.to_string(),
            line: issue.pos.start_line,
            case: issue.kind.to_string(),
        }
    }
}
