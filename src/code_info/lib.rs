pub mod aliases;
pub mod analysis_result;
pub mod assertion;
pub mod ast;
pub mod ast_signature;
pub mod attribute_info;
pub mod class_constant_info;
pub mod class_type_alias;
pub mod classlike_info;
pub mod code_location;
pub mod codebase_info;
pub mod data_flow;
pub mod diff;
pub mod enum_case_info;
pub mod file_info;
pub mod function_context;
pub mod functionlike_identifier;
pub mod functionlike_info;
pub mod functionlike_parameter;
pub mod issue;
pub mod member_visibility;
pub mod method_identifier;
pub mod method_info;
pub mod property_info;
pub mod symbol_references;
pub mod t_atomic;
pub mod t_union;
pub mod taint;
pub mod type_definition_info;
pub mod type_resolution;

use std::collections::BTreeMap;

use code_location::FilePath;
use oxidized::{prim_defs::Comment, tast::Pos};

#[derive(Clone)]
pub struct FileSource<'a> {
    pub file_path: FilePath,
    pub file_path_actual: String,
    pub file_contents: String,
    pub is_production_code: bool,
    pub hh_fixmes: &'a BTreeMap<isize, BTreeMap<isize, Pos>>,
    pub comments: &'a Vec<(Pos, Comment)>,
}

pub const EFFECT_PURE: u8 = 0b00000000;
pub const EFFECT_WRITE_LOCAL: u8 = 0b00000001;
pub const EFFECT_READ_PROPS: u8 = 0b00000010;
pub const EFFECT_READ_GLOBALS: u8 = 0b00000100;
pub const EFFECT_WRITE_PROPS: u8 = 0b00001000;
pub const EFFECT_WRITE_GLOBALS: u8 = 0b0010000;
pub const EFFECT_IMPURE: u8 =
    EFFECT_READ_PROPS | EFFECT_READ_GLOBALS | EFFECT_WRITE_PROPS | EFFECT_WRITE_GLOBALS;
