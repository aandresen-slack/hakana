use super::{type_comparison_result::TypeComparisonResult, union_type_comparator};
use crate::{get_mixed_any, template};
use hakana_reflection_info::{codebase_info::CodebaseInfo, t_atomic::TAtomic, t_union::TUnion};

pub(crate) fn is_contained_by(
    codebase: &CodebaseInfo,
    input_type_part: &TAtomic,
    container_type_part: &TAtomic,
    allow_interface_equality: bool,
    atomic_comparison_result: &mut TypeComparisonResult,
) -> bool {
    let mut all_types_contain = true;

    let input_name = match input_type_part {
        TAtomic::TNamedObject {
            name: input_name, ..
        } => input_name,
        _ => {
            return false;
        }
    };

    let (container_name, container_remapped_params) = match container_type_part {
        TAtomic::TNamedObject {
            name: container_name,
            remapped_params: container_remapped_params,
            ..
        } => (container_name, container_remapped_params),
        _ => panic!(),
    };

    if !codebase.class_or_interface_or_enum_or_trait_exists(input_name) {
        println!("Classlike {} does not exist", input_name);
        return false;
    }

    if !codebase.class_or_interface_or_enum_or_trait_exists(container_name) {
        println!("Classlike {} does not exist", container_name);
        return false;
    }

    let container_type_params = match container_type_part {
        TAtomic::TNamedObject {
            type_params: Some(type_params),
            ..
        } => type_params,
        _ => panic!(),
    };

    // handle case where input named object has no generic params
    if let TAtomic::TNamedObject {
        type_params: None, ..
    } = input_type_part
    {
        if codebase.class_exists(input_name) {
            let class_storage = codebase.classlike_infos.get(input_name).unwrap();

            let mut input_type_part = input_type_part.clone();

            if let Some(extended_params) =
                class_storage.template_extended_params.get(container_name)
            {
                if let TAtomic::TNamedObject {
                    ref mut type_params,
                    ..
                } = input_type_part
                {
                    *type_params = Some(extended_params.values().cloned().collect());
                }
            } else {
                if let TAtomic::TNamedObject {
                    ref mut type_params,
                    ..
                } = input_type_part
                {
                    *type_params = Some(vec![get_mixed_any(); container_type_params.len()]);
                }
            }

            return self::is_contained_by(
                codebase,
                &input_type_part,
                container_type_part,
                allow_interface_equality,
                atomic_comparison_result,
            );
        }

        return false;
    }

    let input_type_params = template::standin_type_replacer::get_mapped_generic_type_params(
        codebase,
        input_type_part,
        container_name,
        *container_remapped_params,
    );

    let container_type_params = match container_type_part {
        TAtomic::TNamedObject {
            type_params: Some(type_params),
            ..
        } => type_params,
        _ => panic!(),
    };

    for (i, input_param) in input_type_params.iter().enumerate() {
        if let Some(container_param) = container_type_params.get(i) {
            compare_generic_params(
                codebase,
                input_type_part,
                input_name,
                input_param,
                container_name,
                container_param,
                i,
                allow_interface_equality,
                &mut all_types_contain,
                atomic_comparison_result,
            );
        } else {
            break;
        }
    }

    if all_types_contain {
        return true;
    }

    false
}

pub(crate) fn compare_generic_params(
    codebase: &CodebaseInfo,
    input_type_part: &TAtomic,
    input_name: &String,
    input_param: &TUnion,
    container_name: &String,
    container_param: &TUnion,
    param_offset: usize,
    allow_interface_equality: bool,
    all_types_contain: &mut bool,
    atomic_comparison_result: &mut TypeComparisonResult,
) {
    if input_param.is_nothing() || input_param.is_placeholder() {
        if let None = atomic_comparison_result.replacement_atomic_type {
            atomic_comparison_result.replacement_atomic_type = Some(input_type_part.clone());
        }

        if let Some(TAtomic::TNamedObject {
            type_params: Some(ref mut type_params),
            ..
        }) = atomic_comparison_result.replacement_atomic_type
        {
            if let Some(existing_param) = type_params.get_mut(param_offset) {
                *existing_param = container_param.clone();
            }
        }

        return;
    }

    let mut param_comparison_result = TypeComparisonResult::new();

    if !union_type_comparator::is_contained_by(
        codebase,
        input_param,
        container_param,
        false,
        false,
        allow_interface_equality,
        &mut param_comparison_result,
    ) {
        if input_name == "Generator"
            && param_offset == 2
            && param_comparison_result
                .type_coerced_from_nested_mixed
                .unwrap_or(false)
        {
            return;
        }

        atomic_comparison_result.type_coerced =
            Some(if let Some(val) = atomic_comparison_result.type_coerced {
                val
            } else {
                param_comparison_result.type_coerced.unwrap_or(false) == true
            });

        atomic_comparison_result.type_coerced_from_nested_mixed = Some(
            if let Some(val) = atomic_comparison_result.type_coerced_from_nested_mixed {
                val
            } else {
                param_comparison_result
                    .type_coerced_from_nested_mixed
                    .unwrap_or(false)
                    == true
            },
        );

        atomic_comparison_result.type_coerced_from_nested_any = Some(
            if let Some(val) = atomic_comparison_result.type_coerced_from_nested_any {
                val
            } else {
                param_comparison_result
                    .type_coerced_from_nested_any
                    .unwrap_or(false)
                    == true
            },
        );

        atomic_comparison_result.type_coerced_from_as_mixed = Some(
            if let Some(val) = atomic_comparison_result.type_coerced_from_as_mixed {
                val
            } else {
                param_comparison_result
                    .type_coerced_from_as_mixed
                    .unwrap_or(false)
                    == true
            },
        );

        atomic_comparison_result.type_coerced_to_literal = Some(
            if let Some(val) = atomic_comparison_result.type_coerced_to_literal {
                val
            } else {
                param_comparison_result
                    .type_coerced_to_literal
                    .unwrap_or(false)
                    == true
            },
        );

        if !param_comparison_result
            .type_coerced_from_as_mixed
            .unwrap_or(false)
        {
            *all_types_contain = false;
        }
    } else if !container_param.has_template() && !input_param.has_template() {
        if input_param.is_literal_of(container_param) {
            if let None = atomic_comparison_result.replacement_atomic_type {
                atomic_comparison_result.replacement_atomic_type = Some(input_type_part.clone());
            }

            if let Some(TAtomic::TNamedObject {
                type_params: Some(ref mut type_params),
                ..
            }) = atomic_comparison_result.replacement_atomic_type
            {
                type_params.insert(param_offset, container_param.clone());
            }
        } else {
            let container_type_params_covariant = if let Some(container_classlike_storage) =
                codebase.classlike_infos.get(container_name)
            {
                container_classlike_storage
                    .template_covariants
                    .contains(&param_offset)
            } else {
                true
            };

            if !container_type_params_covariant && !container_param.had_template {
                if !union_type_comparator::is_contained_by(
                    codebase,
                    container_param,
                    input_param,
                    false,
                    false,
                    allow_interface_equality,
                    &mut param_comparison_result,
                ) || param_comparison_result.type_coerced.unwrap_or(false)
                {
                    if !container_param.has_static_object() || !input_param.is_static_object() {
                        let mut mixed_from_any = false;
                        if container_param.is_mixed_with_any(&mut mixed_from_any)
                            || container_param.is_arraykey()
                        {
                            atomic_comparison_result.type_coerced_from_nested_mixed = Some(true);
                            if mixed_from_any {
                                atomic_comparison_result.type_coerced_from_nested_any = Some(true);
                            }
                        } else {
                            *all_types_contain = false;
                        }

                        atomic_comparison_result.type_coerced = Some(false);
                    }
                }
            }
        }
    }
}
