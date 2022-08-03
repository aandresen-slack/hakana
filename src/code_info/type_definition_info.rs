use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{t_union::TUnion, taint::TaintType};

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeDefinitionInfo {
    pub is_newtype: bool,
    pub as_type: Option<TUnion>,
    pub actual_type: TUnion,

    /**
     * An array holding the function template "as" types.
     *
     * It's the de-facto list of all templates on a given function.
     *
     * The name of the template is the first key. The nested array is keyed by a unique
     * function identifier. This allows operations with the same-named template defined
     * across multiple classes and/or functions to not run into trouble.
     */
    pub template_types: IndexMap<String, HashMap<String, TUnion>>,

    pub shape_field_taints: Option<HashMap<String, HashSet<TaintType>>>,
}
