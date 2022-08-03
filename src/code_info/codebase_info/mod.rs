pub mod symbols;

use self::symbols::SymbolKind;
pub use self::symbols::Symbols;
use crate::class_constant_info::ConstantInfo;
use crate::classlike_info::ClassLikeInfo;
use crate::functionlike_info::FunctionLikeInfo;
use crate::t_atomic::TAtomic;
use crate::t_union::TUnion;
use crate::type_definition_info::TypeDefinitionInfo;
use function_context::method_identifier::MethodIdentifier;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Serialize, Deserialize)]
pub struct CodebaseInfo {
    pub classlike_infos: HashMap<String, ClassLikeInfo>,
    pub functionlike_infos: HashMap<String, FunctionLikeInfo>,
    pub type_definitions: HashMap<String, TypeDefinitionInfo>,
    pub symbols: Symbols,
    pub infer_types_from_usage: bool,
    pub register_stub_files: bool,
    pub constant_infos: HashMap<String, ConstantInfo>,
    pub classlikes_in_files: HashMap<String, HashSet<String>>,
    pub typedefs_in_files: HashMap<String, HashSet<String>>,
    pub functions_in_files: HashMap<String, HashSet<String>>,
    pub const_files: HashMap<String, HashSet<String>>,
    pub classlike_descendents: HashMap<String, HashSet<String>>,
}

impl CodebaseInfo {
    pub fn new() -> Self {
        Self {
            classlike_infos: HashMap::new(),
            functionlike_infos: HashMap::new(),
            symbols: Symbols::new(),
            type_definitions: HashMap::new(),
            infer_types_from_usage: false,
            register_stub_files: false,
            constant_infos: HashMap::new(),
            classlikes_in_files: HashMap::new(),
            typedefs_in_files: HashMap::new(),
            functions_in_files: HashMap::new(),
            const_files: HashMap::new(),
            classlike_descendents: HashMap::new(),
        }
    }

    pub fn class_or_interface_exists(&self, fq_class_name: &String) -> bool {
        match self.symbols.all.get(fq_class_name) {
            Some(SymbolKind::Class | SymbolKind::Interface) => true,
            _ => false,
        }
    }

    pub fn class_or_interface_or_enum_exists(&self, fq_class_name: &String) -> bool {
        match self.symbols.all.get(fq_class_name) {
            Some(SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum) => true,
            _ => false,
        }
    }

    pub fn class_or_interface_or_enum_or_trait_exists(&self, fq_class_name: &String) -> bool {
        match self.symbols.all.get(fq_class_name) {
            Some(
                SymbolKind::Class | SymbolKind::Interface | SymbolKind::Enum | SymbolKind::Trait,
            ) => true,
            _ => false,
        }
    }

    pub fn class_exists(&self, fq_class_name: &String) -> bool {
        match self.symbols.all.get(fq_class_name) {
            Some(SymbolKind::Class) => true,
            _ => false,
        }
    }

    pub fn interface_exists(&self, fq_class_name: &String) -> bool {
        match self.symbols.all.get(fq_class_name) {
            Some(SymbolKind::Interface) => true,
            _ => false,
        }
    }

    pub fn typedef_exists(&self, fq_alias_name: &String) -> bool {
        match self.symbols.all.get(fq_alias_name) {
            Some(SymbolKind::TypeDefinition) => true,
            _ => false,
        }
    }

    pub fn class_extends(&self, child_class: &String, parent_class: &String) -> bool {
        if let Some(classlike_storage) = self.classlike_infos.get(child_class) {
            return classlike_storage.all_parent_classes.contains(parent_class);
        }
        false
    }

    pub fn class_extends_or_implements(&self, child_class: &String, parent_class: &String) -> bool {
        self.class_extends(child_class, parent_class)
            || self.class_implements(child_class, parent_class)
    }

    pub fn interface_extends(&self, child_class: &String, parent_class: &String) -> bool {
        if let Some(classlike_storage) = self.classlike_infos.get(child_class) {
            return classlike_storage
                .all_parent_interfaces
                .contains(parent_class);
        }
        false
    }

    pub fn class_implements(&self, child_class: &String, parent_class: &String) -> bool {
        if let Some(classlike_storage) = self.classlike_infos.get(child_class) {
            return classlike_storage
                .all_class_interfaces
                .contains(parent_class);
        }
        false
    }

    pub fn get_class_constant_type(
        &self,
        fq_class_name: &String,
        const_name: &String,
        _visited_constant_ids: HashSet<String>,
    ) -> Option<TUnion> {
        if let Some(classlike_storage) = self.classlike_infos.get(fq_class_name) {
            if let Some(constant_storage) = classlike_storage.constants.get(const_name) {
                let mut constant_type = if let Some(inferred_type) = &constant_storage.inferred_type
                {
                    Some(inferred_type.clone())
                } else if let Some(provided_type) = &constant_storage.provided_type {
                    Some(provided_type.clone())
                } else {
                    // todo could resolve constant types here
                    None
                };

                if matches!(classlike_storage.kind, SymbolKind::Enum) {
                    if let Some(ref mut constant_type) = constant_type {
                        *constant_type = TUnion::new(vec![TAtomic::TEnumLiteralCase {
                            enum_name: classlike_storage.name.clone(),
                            member_name: const_name.clone(),
                        }]);
                    } else {
                        constant_type = Some(TUnion::new(vec![TAtomic::TEnumLiteralCase {
                            enum_name: classlike_storage.name.clone(),
                            member_name: const_name.clone(),
                        }]));
                    }
                }

                constant_type
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn property_exists(&self, classlike_name: &String, property_name: &String) -> bool {
        if let Some(classlike_info) = self.classlike_infos.get(classlike_name) {
            classlike_info
                .declaring_property_ids
                .contains_key(property_name)
        } else {
            false
        }
    }

    pub fn method_exists(&self, classlike_name: &String, method_name: &String) -> bool {
        if let Some(classlike_info) = self.classlike_infos.get(classlike_name) {
            classlike_info
                .declaring_method_ids
                .contains_key(method_name)
        } else {
            false
        }
    }

    pub fn get_declaring_class_for_property(
        &self,
        fq_class_name: &String,
        property_name: &String,
    ) -> Option<&String> {
        if let Some(classlike_storage) = self.classlike_infos.get(fq_class_name) {
            return classlike_storage.declaring_property_ids.get(property_name);
        }

        return None;
    }

    pub fn get_property_type(
        &self,
        fq_class_name: &String,
        property_name: &String,
    ) -> Option<TUnion> {
        if let Some(classlike_storage) = self.classlike_infos.get(fq_class_name) {
            let declaring_property_class =
                classlike_storage.declaring_property_ids.get(property_name);

            let storage = if let Some(declaring_property_class) = declaring_property_class {
                let declaring_classlike_storage =
                    self.classlike_infos.get(declaring_property_class).unwrap();
                if let Some(val) = declaring_classlike_storage.properties.get(property_name) {
                    Some(val)
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(storage) = storage {
                return Some(storage.type_.clone());
            }

            if let Some(overriden_properties) =
                classlike_storage.overridden_property_ids.get(property_name)
            {
                for overriden_property in overriden_properties {
                    if let Some(_overridden_storage) = self.classlike_infos.get(overriden_property)
                    {
                        // TODO handle overriden property types
                    }
                }
            }
        }

        None
    }

    pub fn get_declaring_method_id(&self, method_id: &MethodIdentifier) -> MethodIdentifier {
        if let Some(classlike_storage) = self.classlike_infos.get(&method_id.0) {
            let classlike_name = classlike_storage
                .declaring_method_ids
                .get(&method_id.1)
                .cloned()
                .unwrap_or(method_id.0.clone());
            return MethodIdentifier(classlike_name, method_id.1.clone());
        }

        method_id.clone()
    }

    pub fn get_appearing_method_id(&self, method_id: &MethodIdentifier) -> MethodIdentifier {
        if let Some(classlike_storage) = self.classlike_infos.get(&method_id.0) {
            let classlike_name = classlike_storage
                .appearing_method_ids
                .get(&method_id.1)
                .cloned()
                .unwrap_or(method_id.0.clone());
            return MethodIdentifier(classlike_name, method_id.1.clone());
        }

        method_id.clone()
    }

    pub fn get_method(&self, method_id: &MethodIdentifier) -> Option<&FunctionLikeInfo> {
        if let Some(classlike_storage) = self.classlike_infos.get(&method_id.0) {
            return classlike_storage.methods.get(&method_id.1);
        }

        None
    }

    pub fn extend(&mut self, other: CodebaseInfo) {
        self.classlike_infos.extend(other.classlike_infos);
        self.functionlike_infos.extend(other.functionlike_infos);
        self.symbols.all.extend(other.symbols.all);
        self.symbols
            .classlike_files
            .extend(other.symbols.classlike_files);
        self.type_definitions.extend(other.type_definitions);
        self.constant_infos.extend(other.constant_infos);
        self.classlikes_in_files.extend(other.classlikes_in_files);
        self.typedefs_in_files.extend(other.typedefs_in_files);
        self.functions_in_files.extend(other.functions_in_files);
        self.const_files.extend(other.const_files);
    }
}
