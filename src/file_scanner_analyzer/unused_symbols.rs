use hakana_analyzer::config::Config;
use hakana_reflection_info::analysis_result::{AnalysisResult, Replacement};
use hakana_reflection_info::classlike_info::ClassLikeInfo;
use hakana_reflection_info::codebase_info::symbols::SymbolKind;
use hakana_reflection_info::codebase_info::{CodebaseInfo, Symbols};
use hakana_reflection_info::functionlike_identifier::FunctionLikeIdentifier;
use hakana_reflection_info::issue::{Issue, IssueKind};
use hakana_reflection_info::member_visibility::MemberVisibility;
use hakana_reflection_info::property_info::PropertyKind;
use hakana_reflection_info::{Interner, StrId, STR_CONSTRUCT, STR_EMPTY};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) fn find_unused_definitions(
    analysis_result: &mut AnalysisResult,
    config: &Arc<Config>,
    codebase: &CodebaseInfo,
    interner: &Interner,
    ignored_paths: &Option<FxHashSet<String>>,
) {
    let referenced_symbols_and_members = analysis_result
        .symbol_references
        .get_referenced_symbols_and_members();
    let referenced_overridden_class_members = analysis_result
        .symbol_references
        .get_referenced_overridden_class_members();

    'outer1: for (functionlike_name, functionlike_info) in &codebase.functionlike_infos {
        if functionlike_name.1 == STR_EMPTY
            && functionlike_info.user_defined
            && !functionlike_info.dynamically_callable
            && !functionlike_info.generated
        {
            let pos = functionlike_info.name_location.as_ref().unwrap();
            let file_path = interner.lookup(&pos.file_path.0);

            if let Some(ignored_paths) = ignored_paths {
                for ignored_path in ignored_paths {
                    if file_path.matches(ignored_path.as_str()).count() > 0 {
                        continue 'outer1;
                    }
                }
            }

            if !referenced_symbols_and_members.contains(&functionlike_name) {
                if let Some(suppressed_issues) = &functionlike_info.suppressed_issues {
                    if suppressed_issues.contains_key(&IssueKind::UnusedFunction) {
                        continue;
                    }
                }

                if !config.allow_issue_kind_in_file(&IssueKind::UnusedFunction, &file_path) {
                    continue;
                }

                if config.migration_symbols.contains(&(
                    "unused_symbol".to_string(),
                    interner.lookup(&functionlike_name.0).to_string(),
                )) {
                    let def_pos = &functionlike_info.def_location;
                    analysis_result
                        .replacements
                        .entry(pos.file_path)
                        .or_insert_with(BTreeMap::new)
                        .insert(
                            (def_pos.start_offset, def_pos.end_offset),
                            Replacement::TrimPrecedingWhitespace(
                                (def_pos.start_offset - (def_pos.start_column - 1)) as u64,
                            ),
                        );
                }

                let issue = Issue::new(
                    IssueKind::UnusedFunction,
                    format!("Unused function {}", interner.lookup(&functionlike_name.0)),
                    pos.clone(),
                    &Some(FunctionLikeIdentifier::Function(functionlike_name.0)),
                );

                if config.can_add_issue(&issue) {
                    *analysis_result
                        .issue_counts
                        .entry(issue.kind.clone())
                        .or_insert(0) += 1;
                    analysis_result
                        .emitted_definition_issues
                        .entry(pos.file_path)
                        .or_insert_with(Vec::new)
                        .push(issue);
                }
            }
        }
    }

    'outer2: for (classlike_name, classlike_info) in &codebase.classlike_infos {
        if classlike_info.user_defined && !classlike_info.generated {
            let pos = &classlike_info.name_location;
            let file_path = interner.lookup(&pos.file_path.0);

            if let Some(ignored_paths) = ignored_paths {
                for ignored_path in ignored_paths {
                    if file_path.matches(ignored_path.as_str()).count() > 0 {
                        continue 'outer2;
                    }
                }
            }

            for parent_class in &classlike_info.all_parent_classes {
                if let Some(parent_classlike_info) = codebase.classlike_infos.get(parent_class) {
                    if !parent_classlike_info.user_defined {
                        continue 'outer2;
                    }
                }
            }

            if !referenced_symbols_and_members.contains(&(*classlike_name, STR_EMPTY)) {
                if !config.allow_issue_kind_in_file(&IssueKind::UnusedClass, &file_path) {
                    continue;
                }

                let issue = Issue::new(
                    IssueKind::UnusedClass,
                    format!(
                        "Unused class, interface or enum {}",
                        interner.lookup(classlike_name),
                    ),
                    pos.clone(),
                    &Some(FunctionLikeIdentifier::Function(*classlike_name)),
                );

                if config.migration_symbols.contains(&(
                    "unused_symbol".to_string(),
                    interner.lookup(classlike_name).to_string(),
                )) {
                    let def_pos = &classlike_info.def_location;
                    analysis_result
                        .replacements
                        .entry(pos.file_path)
                        .or_insert_with(BTreeMap::new)
                        .insert(
                            (def_pos.start_offset, def_pos.end_offset),
                            Replacement::TrimPrecedingWhitespace(
                                (def_pos.start_offset - (def_pos.start_column - 1)) as u64,
                            ),
                        );
                }

                if config.can_add_issue(&issue) {
                    *analysis_result
                        .issue_counts
                        .entry(issue.kind.clone())
                        .or_insert(0) += 1;
                    analysis_result
                        .emitted_definition_issues
                        .entry(pos.file_path)
                        .or_insert_with(Vec::new)
                        .push(issue);
                }
            } else {
                'inner: for method_name_ptr in &classlike_info.methods {
                    if *method_name_ptr != STR_EMPTY {
                        let method_name = interner.lookup(method_name_ptr);

                        if method_name.starts_with("__") {
                            continue;
                        }
                    }

                    let pair = (classlike_name.clone(), *method_name_ptr);

                    if !referenced_symbols_and_members.contains(&pair)
                        && !referenced_overridden_class_members.contains(&pair)
                    {
                        if has_upstream_method_call(
                            classlike_info,
                            method_name_ptr,
                            &referenced_symbols_and_members,
                        ) {
                            continue;
                        }

                        for descendant_classlike in codebase.get_all_descendants(classlike_name) {
                            if let Some(descendant_classlike_storage) =
                                codebase.classlike_infos.get(&descendant_classlike)
                            {
                                for parent_interface in
                                    &descendant_classlike_storage.all_class_interfaces
                                {
                                    if referenced_symbols_and_members
                                        .contains(&(*parent_interface, *method_name_ptr))
                                    {
                                        continue 'inner;
                                    }
                                }
                            }
                        }

                        for trait_user in get_trait_users(
                            classlike_name,
                            &codebase.symbols,
                            &codebase.classlike_descendants,
                        ) {
                            if let Some(classlike_info) = codebase.classlike_infos.get(&trait_user)
                            {
                                if has_upstream_method_call(
                                    classlike_info,
                                    method_name_ptr,
                                    &referenced_symbols_and_members,
                                ) {
                                    continue 'inner;
                                }
                            }
                        }

                        let functionlike_storage = codebase
                            .functionlike_infos
                            .get(&(*classlike_name, *method_name_ptr))
                            .unwrap();

                        let method_storage = functionlike_storage.method_info.as_ref().unwrap();

                        if let Some(suppressed_issues) = &functionlike_storage.suppressed_issues {
                            if suppressed_issues.contains_key(&IssueKind::UnusedPrivateMethod) {
                                continue;
                            }
                        }

                        // allow one-liner private construct statements that prevent instantiation
                        if *method_name_ptr == STR_CONSTRUCT
                            && matches!(method_storage.visibility, MemberVisibility::Private)
                        {
                            let stmt_pos = &functionlike_storage.def_location;
                            if let Some(name_pos) = &functionlike_storage.name_location {
                                if stmt_pos.end_line - name_pos.start_line <= 1 {
                                    continue;
                                }
                            }
                        }

                        let issue =
                            if matches!(method_storage.visibility, MemberVisibility::Private) {
                                Issue::new(
                                    IssueKind::UnusedPrivateMethod,
                                    format!(
                                        "Unused method {}::{}",
                                        interner.lookup(classlike_name),
                                        interner.lookup(method_name_ptr)
                                    ),
                                    functionlike_storage.name_location.clone().unwrap(),
                                    &Some(FunctionLikeIdentifier::Method(
                                        *classlike_name,
                                        *method_name_ptr,
                                    )),
                                )
                            } else {
                                Issue::new(
                                    IssueKind::UnusedPublicOrProtectedMethod,
                                    format!(
                                        "Possibly-unused method {}::{}",
                                        interner.lookup(classlike_name),
                                        interner.lookup(method_name_ptr)
                                    ),
                                    functionlike_storage.name_location.clone().unwrap(),
                                    &Some(FunctionLikeIdentifier::Method(
                                        *classlike_name,
                                        *method_name_ptr,
                                    )),
                                )
                            };

                        let file_path = interner.lookup(&pos.file_path.0);

                        if !config.allow_issue_kind_in_file(&issue.kind, &file_path) {
                            continue;
                        }

                        if config.can_add_issue(&issue) {
                            *analysis_result
                                .issue_counts
                                .entry(issue.kind.clone())
                                .or_insert(0) += 1;
                            analysis_result
                                .emitted_definition_issues
                                .entry(pos.file_path)
                                .or_insert_with(Vec::new)
                                .push(issue);
                        }
                    }
                }

                for (property_name_ptr, property_storage) in &classlike_info.properties {
                    let pair = (classlike_name.clone(), *property_name_ptr);

                    if !referenced_symbols_and_members.contains(&pair)
                        && !referenced_overridden_class_members.contains(&pair)
                    {
                        if let Some(suppressed_issues) = &property_storage.suppressed_issues {
                            if suppressed_issues.contains_key(&IssueKind::UnusedPrivateProperty) {
                                continue;
                            }
                        }

                        let issue =
                            if matches!(property_storage.visibility, MemberVisibility::Private) {
                                Issue::new(
                                    IssueKind::UnusedPrivateProperty,
                                    format!(
                                        "Unused private property {}::${}",
                                        interner.lookup(classlike_name),
                                        interner.lookup(property_name_ptr)
                                    ),
                                    property_storage.pos.clone().unwrap(),
                                    &Some(FunctionLikeIdentifier::Method(
                                        *classlike_name,
                                        *property_name_ptr,
                                    )),
                                )
                            } else if let PropertyKind::XhpAttribute { .. } = property_storage.kind
                            {
                                Issue::new(
                                    IssueKind::UnusedXhpAttribute,
                                    format!(
                                        "Unused XHP attribute {} in class {}",
                                        interner.lookup(property_name_ptr),
                                        interner.lookup(classlike_name),
                                    ),
                                    property_storage.pos.clone().unwrap(),
                                    &Some(FunctionLikeIdentifier::Method(
                                        *classlike_name,
                                        *property_name_ptr,
                                    )),
                                )
                            } else {
                                Issue::new(
                                    IssueKind::UnusedPublicOrProtectedProperty,
                                    format!(
                                        "Unused public or protected property {}::${}",
                                        interner.lookup(classlike_name),
                                        interner.lookup(property_name_ptr)
                                    ),
                                    property_storage.pos.clone().unwrap(),
                                    &Some(FunctionLikeIdentifier::Method(
                                        *classlike_name,
                                        *property_name_ptr,
                                    )),
                                )
                            };

                        let file_path = interner.lookup(&pos.file_path.0);

                        if !config.allow_issue_kind_in_file(&issue.kind, &file_path) {
                            continue;
                        }

                        if config.issues_to_fix.contains(&issue.kind) && !config.add_fixmes {
                            if let Some(stmt_pos) = property_storage.stmt_pos {
                                analysis_result
                                    .replacements
                                    .entry(pos.file_path)
                                    .or_insert_with(BTreeMap::new)
                                    .insert(
                                        (stmt_pos.start_offset, stmt_pos.end_offset),
                                        Replacement::TrimPrecedingWhitespace(
                                            (stmt_pos.start_offset - stmt_pos.start_column) as u64,
                                        ),
                                    );
                            }
                        } else if config.can_add_issue(&issue) {
                            *analysis_result
                                .issue_counts
                                .entry(issue.kind.clone())
                                .or_insert(0) += 1;
                            analysis_result
                                .emitted_definition_issues
                                .entry(pos.file_path)
                                .or_insert_with(Vec::new)
                                .push(issue);
                        }
                    }
                }
            }
        }
    }

    'outer2: for (type_name, type_definition_info) in &codebase.type_definitions {
        if type_definition_info.user_defined && !type_definition_info.generated {
            let pos = &type_definition_info.location;
            let file_path = interner.lookup(&pos.file_path.0);

            if let Some(ignored_paths) = ignored_paths {
                for ignored_path in ignored_paths {
                    if file_path.matches(ignored_path.as_str()).count() > 0 {
                        continue 'outer2;
                    }
                }
            }

            if !config.allow_issue_kind_in_file(&IssueKind::UnusedTypeDefinition, &file_path) {
                continue;
            }

            if !referenced_symbols_and_members.contains(&(*type_name, STR_EMPTY)) {
                let issue = Issue::new(
                    IssueKind::UnusedTypeDefinition,
                    format!("Unused type definition {}", interner.lookup(type_name)),
                    pos.clone(),
                    &Some(FunctionLikeIdentifier::Function(*type_name)),
                );

                if config.migration_symbols.contains(&(
                    "unused_symbol".to_string(),
                    interner.lookup(type_name).to_string(),
                )) {
                    analysis_result
                        .replacements
                        .entry(pos.file_path)
                        .or_insert_with(BTreeMap::new)
                        .insert(
                            (pos.start_offset, pos.end_offset),
                            Replacement::TrimPrecedingWhitespace(
                                (pos.start_offset - (pos.start_column - 1)) as u64,
                            ),
                        );
                }

                if config.can_add_issue(&issue) {
                    *analysis_result
                        .issue_counts
                        .entry(issue.kind.clone())
                        .or_insert(0) += 1;
                    analysis_result
                        .emitted_definition_issues
                        .entry(pos.file_path)
                        .or_insert_with(Vec::new)
                        .push(issue);
                }
            }
        }
    }
}

fn has_upstream_method_call(
    classlike_info: &ClassLikeInfo,
    method_name_ptr: &StrId,
    referenced_class_members: &FxHashSet<&(StrId, StrId)>,
) -> bool {
    if let Some(parent_elements) = classlike_info.overridden_method_ids.get(method_name_ptr) {
        for parent_element in parent_elements {
            if referenced_class_members.contains(&(*parent_element, *method_name_ptr)) {
                return true;
            }
        }
    }

    return false;
}

fn get_trait_users(
    classlike_name: &StrId,
    symbols: &Symbols,
    all_classlike_descendants: &FxHashMap<StrId, FxHashSet<StrId>>,
) -> FxHashSet<StrId> {
    let mut base_set = FxHashSet::default();

    if let Some(SymbolKind::Trait) = symbols.all.get(classlike_name) {
        if let Some(classlike_descendants) = all_classlike_descendants.get(classlike_name) {
            base_set.extend(classlike_descendants);
            for classlike_descendant in classlike_descendants {
                base_set.extend(get_trait_users(
                    classlike_descendant,
                    symbols,
                    all_classlike_descendants,
                ));
            }
        }
    }

    base_set
}
