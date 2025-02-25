use rustc_hash::FxHashMap;

use crate::{codebase_info::CodebaseInfo, Interner, StrId};

pub fn get_id_name(
    id: &oxidized::ast_defs::Id,
    calling_class: &Option<StrId>,
    calling_class_final: bool,
    codebase: &CodebaseInfo,
    is_static: &mut bool,
    resolved_names: &FxHashMap<usize, StrId>,
) -> Option<StrId> {
    Some(match id.1.as_str() {
        "self" => {
            let self_name = if let Some(calling_class) = calling_class {
                calling_class
            } else {
                return None;
            };

            *self_name
        }
        "parent" => {
            let self_name = if let Some(calling_class) = calling_class {
                calling_class
            } else {
                return None;
            };

            let classlike_storage = codebase.classlike_infos.get(self_name).unwrap();
            classlike_storage.direct_parent_class.unwrap()
        }
        "static" => {
            if !calling_class_final {
                *is_static = true;
            }

            let self_name = if let Some(calling_class) = calling_class {
                calling_class
            } else {
                return None;
            };

            *self_name
        }
        _ => {
            if let Some(resolved_name) = resolved_names.get(&id.0.start_offset()) {
                *resolved_name
            } else {
                // this is bad
                return None;
            }
        }
    })
}

pub fn get_id_str_name<'a>(
    id: &'a str,
    calling_class: &Option<StrId>,
    codebase: &'a CodebaseInfo,
    interner: &'a Interner,
) -> Option<&'a str> {
    Some(match id {
        "self" => {
            let self_name = if let Some(calling_class) = calling_class {
                calling_class
            } else {
                return None;
            };

            interner.lookup(self_name)
        }
        "parent" => {
            let self_name = if let Some(calling_class) = calling_class {
                calling_class
            } else {
                return None;
            };

            let classlike_storage = codebase.classlike_infos.get(self_name).unwrap();
            interner.lookup(&classlike_storage.direct_parent_class.unwrap())
        }
        "static" => {
            return None;
        }
        _ => id,
    })
}
