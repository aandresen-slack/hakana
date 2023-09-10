pub(crate) mod populator;

use analyzer::analyze_files;
use diff::mark_safe_symbols_from_diff;
use file::{FileStatus, VirtualFileSystem};
use hakana_aast_helper::{get_aast_for_path_and_contents, ParserError};
use hakana_analyzer::config::Config;
use hakana_analyzer::dataflow::program_analyzer::{find_connections, find_tainted_data};
use hakana_logger::Logger;
use hakana_reflection_info::analysis_result::AnalysisResult;
use hakana_reflection_info::code_location::FilePath;
use hakana_reflection_info::codebase_info::CodebaseInfo;
use hakana_reflection_info::data_flow::graph::{GraphKind, WholeProgramKind};
use hakana_reflection_info::symbol_references::SymbolReferences;
use hakana_reflection_info::{Interner, StrId};
use indicatif::ProgressBar;
use oxidized::aast;
use oxidized::scoured_comments::ScouredComments;
use populator::populate_codebase;
use rust_embed::RustEmbed;
use rustc_hash::{FxHashMap, FxHashSet};
use scanner::{scan_files, ScanFilesResult};
use std::fs;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use unused_symbols::find_unused_definitions;

mod analyzer;
mod ast_differ;
mod cache;
mod diff;
pub mod file;
pub mod scanner;
mod unused_symbols;
pub mod wasm;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../third-party/hhvm/hphp/hack/hhi"]
#[prefix = "hhi_embedded_"]
#[include = "*.hhi"]
#[include = "*.php"]
#[include = "*.hack"]
struct HhiAsset;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../third-party/hhvm/hphp/hsl/src"]
#[prefix = "hsl_embedded_"]
#[include = "*.hhi"]
#[include = "*.php"]
#[include = "*.hack"]
struct HslAsset;

#[derive(Clone, Debug)]
pub struct SuccessfulScanData {
    pub codebase: CodebaseInfo,
    pub interner: Interner,
    pub file_system: VirtualFileSystem,
    pub resolved_names: FxHashMap<FilePath, FxHashMap<usize, StrId>>,
}

impl Default for SuccessfulScanData {
    fn default() -> Self {
        SuccessfulScanData {
            codebase: CodebaseInfo::new(),
            interner: Interner::default(),
            file_system: VirtualFileSystem::default(),
            resolved_names: FxHashMap::default(),
        }
    }
}

pub async fn scan_and_analyze(
    stubs_dirs: Vec<String>,
    filter: Option<String>,
    ignored_paths: Option<FxHashSet<String>>,
    config: Arc<Config>,
    cache_dir: Option<&String>,
    threads: u8,
    logger: Arc<Logger>,
    header: &str,
    previous_scan_data: Option<SuccessfulScanData>,
    previous_analysis_result: Option<AnalysisResult>,
    language_server_changes: Option<FxHashMap<String, FileStatus>>,
) -> io::Result<(AnalysisResult, SuccessfulScanData)> {
    let mut all_scanned_dirs = stubs_dirs.clone();
    all_scanned_dirs.push(config.root_dir.clone());

    let file_discovery_and_scanning_now = Instant::now();

    let ScanFilesResult {
        mut codebase,
        mut interner,
        resolved_names,
        codebase_diff,
        file_system,
        asts,
        mut files_to_analyze,
    } = scan_files(
        &all_scanned_dirs,
        cache_dir,
        &config,
        threads,
        logger.clone(),
        header,
        previous_scan_data,
        language_server_changes,
    )
    .await?;

    let file_discovery_and_scanning_elapsed = file_discovery_and_scanning_now.elapsed();

    if logger.can_log_timing() {
        logger
            .log(&format!(
                "File discovery & scanning took {:.2?}",
                file_discovery_and_scanning_elapsed
            ))
            .await;
    }

    if let Some(cache_dir) = cache_dir {
        let timestamp_path = format!("{}/buildinfo", cache_dir);
        let mut timestamp_file = fs::File::create(&timestamp_path).unwrap();
        write!(timestamp_file, "{}", header).unwrap();

        let aast_manifest_path = format!("{}/manifest", cache_dir);
        fs::File::create(&aast_manifest_path)
            .unwrap()
            .write_all(&bincode::serialize(&file_system).unwrap())
            .unwrap_or_else(|_| panic!("Could not write aast manifest {}", &aast_manifest_path));
    }

    let references_path = if let Some(cache_dir) = cache_dir {
        Some(format!("{}/references", cache_dir))
    } else {
        None
    };

    let issues_path = if let Some(cache_dir) = cache_dir {
        Some(format!("{}/issues", cache_dir))
    } else {
        None
    };

    let mut safe_symbols = FxHashSet::default();
    let mut safe_symbol_members = FxHashSet::default();
    let mut existing_issues = FxHashMap::default();
    let mut symbol_references = SymbolReferences::new();
    let mut safe_symbol_member_signatures = FxHashSet::default();

    if config.ast_diff {
        let cached_analysis = mark_safe_symbols_from_diff(
            &logger,
            codebase_diff,
            &codebase,
            &mut interner,
            &mut files_to_analyze,
            &issues_path,
            &references_path,
            previous_analysis_result,
        )
        .await;

        safe_symbols = cached_analysis.safe_symbols;
        safe_symbol_members = cached_analysis.safe_symbol_members;
        existing_issues = cached_analysis.existing_issues;
        symbol_references = cached_analysis.symbol_references;
        safe_symbol_member_signatures = cached_analysis.safe_symbol_member_signatures;
    }

    logger.log("Calculating symbol inheritance").await;

    let populating_now = Instant::now();

    populate_codebase(
        &mut codebase,
        &interner,
        &mut symbol_references,
        &safe_symbols,
        &safe_symbol_member_signatures,
    );

    let populating_elapsed = populating_now.elapsed();

    if logger.can_log_timing() {
        logger
            .log(&format!(
                "Populating codebase took {:.2?}",
                populating_elapsed
            ))
            .await;
    }

    codebase.safe_symbols = safe_symbols;
    codebase.safe_symbol_members = safe_symbol_members;

    let mut analysis_result = AnalysisResult::new(config.graph_kind, symbol_references);

    analysis_result.emitted_issues = existing_issues;

    let analysis_result = Arc::new(Mutex::new(analysis_result));

    let scan_data = SuccessfulScanData {
        codebase,
        interner,
        file_system,
        resolved_names,
    };

    let arc_scan_data = Arc::new(scan_data);

    let analyzed_files_now = Instant::now();

    analyze_files(
        files_to_analyze,
        arc_scan_data.clone(),
        asts,
        config.clone(),
        &analysis_result,
        filter,
        &ignored_paths,
        threads,
        logger.clone(),
    )
    .await?;

    let analyzed_files_elapsed = analyzed_files_now.elapsed();

    if logger.can_log_timing() {
        logger
            .log(&format!(
                "File analysis took {:.2?}",
                analyzed_files_elapsed
            ))
            .await;
    }

    let mut analysis_result = (*analysis_result.lock().unwrap()).clone();

    analysis_result.time_in_analysis = analyzed_files_elapsed;

    if let Some(references_path) = references_path {
        let mut symbols_file = fs::File::create(&references_path).unwrap();
        let serialized_symbol_references =
            bincode::serialize(&analysis_result.symbol_references).unwrap();
        symbols_file.write_all(&serialized_symbol_references)?;
    }

    if let Some(issues_path) = issues_path {
        let mut issues_file = fs::File::create(&issues_path).unwrap();
        let serialized_issues = bincode::serialize(&analysis_result.emitted_issues).unwrap();
        issues_file.write_all(&serialized_issues)?;
    }

    let scan_data = Arc::try_unwrap(arc_scan_data).unwrap();

    if config.find_unused_definitions {
        find_unused_definitions(
            &mut analysis_result,
            &config,
            &scan_data.codebase,
            &scan_data.interner,
            &ignored_paths,
        );
    }

    if let GraphKind::WholeProgram(whole_program_kind) = config.graph_kind {
        let issues = match whole_program_kind {
            WholeProgramKind::Taint => find_tainted_data(
                &analysis_result.program_dataflow_graph,
                &config,
                &logger,
                &scan_data.interner,
            ),
            WholeProgramKind::Query => find_connections(
                &analysis_result.program_dataflow_graph,
                &config,
                &logger,
                &scan_data.interner,
            ),
        };

        for issue in issues {
            analysis_result
                .emitted_issues
                .entry(issue.pos.file_path)
                .or_insert_with(Vec::new)
                .push(issue);
        }
    }

    Ok((analysis_result, scan_data))
}

pub fn get_aast_for_path(
    file_path: FilePath,
    file_path_str: &str,
) -> Result<(aast::Program<(), ()>, ScouredComments, String), ParserError> {
    let file_contents = if file_path_str.starts_with("hsl_embedded_") {
        std::str::from_utf8(
            &HslAsset::get(file_path_str)
                .unwrap_or_else(|| panic!("Could not read HSL file {}", file_path_str))
                .data,
        )
        .unwrap_or_else(|_| panic!("Could not convert HSL file {}", file_path_str))
        .to_string()
    } else if file_path_str.starts_with("hhi_embedded_") {
        std::str::from_utf8(
            &HhiAsset::get(file_path_str)
                .unwrap_or_else(|| panic!("Could not read HSL file {}", file_path_str))
                .data,
        )
        .unwrap_or_else(|_| panic!("Could not convert HHI file {}", file_path_str))
        .to_string()
    } else {
        match fs::read_to_string(file_path_str) {
            Ok(str_file) => str_file,
            Err(_) => return Err(ParserError::NotAHackFile),
        }
    };

    get_aast_for_path_and_contents(file_path, file_path_str, file_contents)
}

fn update_progressbar(percentage: u64, bar: Option<Arc<ProgressBar>>) {
    if let Some(bar) = bar {
        bar.set_position(percentage);
    }
}
