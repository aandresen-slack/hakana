use crate::classlike_analyzer::ClassLikeAnalyzer;
use crate::custom_hook::AfterDefAnalysisData;
use crate::function_analysis_data::FunctionAnalysisData;
use crate::functionlike_analyzer::FunctionLikeAnalyzer;
use crate::scope_analyzer::ScopeAnalyzer;
use crate::scope_context::loop_scope::LoopScope;
use crate::scope_context::ScopeContext;
use crate::statements_analyzer::StatementsAnalyzer;
use crate::{expression_analyzer, stmt_analyzer};
use hakana_reflection_info::analysis_result::AnalysisResult;
use hakana_reflection_info::function_context::FunctionContext;
use hakana_reflection_info::issue::{Issue, IssueKind};
use oxidized::aast;

pub(crate) fn analyze(
    scope_analyzer: &mut dyn ScopeAnalyzer,
    statements_analyzer: &StatementsAnalyzer,
    def: &aast::Def<(), ()>,
    context: &mut ScopeContext,
    loop_scope: &mut Option<LoopScope>,
    analysis_data: &mut FunctionAnalysisData,
    analysis_result: &mut AnalysisResult,
) {
    match def {
        aast::Def::Fun(fun) => {
            let file_analyzer = scope_analyzer.get_file_analyzer();
            let mut function_analyzer = FunctionLikeAnalyzer::new(file_analyzer);
            function_analyzer.analyze_fun(fun, analysis_result);

            if analysis_data.first_statement_offset.is_none() {
                analysis_data.first_statement_offset =
                    Some(fun.fun.span.first_char_of_line().start_offset());
            }
        }
        aast::Def::Class(class) => {
            let file_analyzer = scope_analyzer.get_file_analyzer();
            let mut class_analyzer = ClassLikeAnalyzer::new(file_analyzer);
            class_analyzer.analyze(&class, statements_analyzer, analysis_result);

            if analysis_data.first_statement_offset.is_none() {
                analysis_data.first_statement_offset =
                    Some(class.span.first_char_of_line().start_offset());
            }
        }
        aast::Def::Typedef(t) => {
            if analysis_data.first_statement_offset.is_none() {
                analysis_data.first_statement_offset =
                    Some(t.name.0.first_char_of_line().start_offset());
            }
        }
        aast::Def::NamespaceUse(u) => {
            if analysis_data.first_statement_offset.is_none() {
                analysis_data.first_statement_offset =
                    Some(u[0].1 .0.first_char_of_line().start_offset());
            }
        }
        aast::Def::Stmt(boxed) => {
            if analysis_data.first_statement_offset.is_none() {
                analysis_data.first_statement_offset = Some(boxed.0.start_offset());
            }

            stmt_analyzer::analyze(
                statements_analyzer,
                boxed,
                analysis_data,
                context,
                loop_scope,
            );
        }
        aast::Def::Constant(boxed) => {
            let mut function_context = FunctionContext::new();
            function_context.calling_class = Some(
                *statements_analyzer
                    .get_file_analyzer()
                    .resolved_names
                    .get(&boxed.name.pos().start_offset())
                    .unwrap(),
            );

            let mut context = ScopeContext::new(function_context);

            expression_analyzer::analyze(
                statements_analyzer,
                &boxed.value,
                analysis_data,
                &mut context,
                &mut None,
            );

            if analysis_data.first_statement_offset.is_none() {
                analysis_data.first_statement_offset =
                    Some(boxed.span.first_char_of_line().start_offset());
            }
        }
        aast::Def::Namespace(_) => {
            // already handled?
        }
        aast::Def::SetNamespaceEnv(_) => {
            // maybe unnecessary
        }
        aast::Def::FileAttributes(_) => {
            // not sure
        }
        aast::Def::Module(boxed) => {
            analysis_data.maybe_add_issue(
                Issue::new(
                    IssueKind::UnrecognizedStatement,
                    "Unrecognized statement".to_string(),
                    statements_analyzer.get_hpos(&boxed.span),
                    &None,
                ),
                statements_analyzer.get_config(),
                statements_analyzer.get_file_path_actual(),
            );
        }
        aast::Def::SetModule(boxed) => {
            analysis_data.maybe_add_issue(
                Issue::new(
                    IssueKind::UnrecognizedStatement,
                    "Unrecognized statement".to_string(),
                    statements_analyzer.get_hpos(boxed.pos()),
                    &None,
                ),
                statements_analyzer.get_config(),
                statements_analyzer.get_file_path_actual(),
            );
        }
    }

    for hook in &statements_analyzer.get_config().hooks {
        hook.after_def_analysis(
            analysis_data,
            AfterDefAnalysisData {
                statements_analyzer,
                def: &def,
                context,
            },
        );
    }
}
