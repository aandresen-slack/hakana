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

use std::{
    collections::BTreeMap,
    hash::BuildHasherDefault,
    sync::{Arc, Mutex},
};

use code_location::FilePath;
use indexmap::{IndexMap, IndexSet};
use oxidized::{prim_defs::Comment, tast::Pos};
use rustc_hash::{self, FxHashMap, FxHasher};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct FileSource<'a> {
    pub file_path: FilePath,
    pub file_path_actual: String,
    pub file_contents: String,
    pub is_production_code: bool,
    pub hh_fixmes: &'a BTreeMap<isize, BTreeMap<isize, Pos>>,
    pub comments: &'a Vec<(Pos, Comment)>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct StrId(pub u32);

impl StrId {
    pub const EMPTY: StrId = StrId(0);
    pub const THIS: StrId = StrId(1);
    pub const ANONYMOUS_FN: StrId = StrId(2);
    pub const ISSET: StrId = StrId(3);
    pub const UNSET: StrId = StrId(4);
    pub const MEMBER_OF: StrId = StrId(5);
    pub const ECHO: StrId = StrId(6);
    pub const CONSTRUCT: StrId = StrId(7);
    pub const DATA_ATTRIBUTE: StrId = StrId(8);
    pub const ARIA_ATTRIBUTE: StrId = StrId(9);
    pub const ANY_ARRAY: StrId = StrId(10);
    pub const KEYED_CONTAINER: StrId = StrId(11);
    pub const CONTAINER: StrId = StrId(12);
    pub const PHP_INCOMPLETE_CLASS: StrId = StrId(13);
    pub const XHP_CHILD: StrId = StrId(14);
    pub const AWAITABLE: StrId = StrId(15);
    pub const BUILTIN_ENUM: StrId = StrId(16);
    pub const BUILTIN_ENUM_CLASS: StrId = StrId(17);
    pub const STATIC: StrId = StrId(18);
    pub const SELF: StrId = StrId(19);
    pub const FORMAT_STRING: StrId = StrId(20);
    pub const ENUM_CLASS_LABEL: StrId = StrId(21);
    pub const TRAVERSABLE: StrId = StrId(22);
    pub const KEYED_TRAVERSABLE: StrId = StrId(23);
    pub const LIB_REGEX_PATTERN: StrId = StrId(24);
    pub const ITERATOR: StrId = StrId(25);
    pub const KEYED_ITERATOR: StrId = StrId(26);
    pub const ASYNC_ITERATOR: StrId = StrId(27);
    pub const ASYNC_KEYED_ITERATOR: StrId = StrId(28);
    pub const SHAPES: StrId = StrId(29);
    pub const STDCLASS: StrId = StrId(30);
    pub const SIMPLE_XML_ELEMENT: StrId = StrId(31);
    pub const ASIO_JOIN: StrId = StrId(32);
    pub const INCLUDE: StrId = StrId(33);
    pub const TYPE_STRUCTURE: StrId = StrId(34);
    pub const INVARIANT: StrId = StrId(35);
    pub const COERCE: StrId = StrId(36);
    pub const ASSERT: StrId = StrId(37);
    pub const ASSERT_ALL: StrId = StrId(38);
    pub const KEY_EXISTS: StrId = StrId(39);
    pub const REMOVE_KEY: StrId = StrId(40);
    pub const MATH_INT32_MAX: StrId = StrId(41);

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

pub const EFFECT_PURE: u8 = 0b00000000;
pub const EFFECT_WRITE_LOCAL: u8 = 0b00000001;
pub const EFFECT_READ_PROPS: u8 = 0b00000010;
pub const EFFECT_READ_GLOBALS: u8 = 0b00000100;
pub const EFFECT_WRITE_PROPS: u8 = 0b00001000;
pub const EFFECT_WRITE_GLOBALS: u8 = 0b0010000;
pub const EFFECT_IMPURE: u8 =
    EFFECT_READ_PROPS | EFFECT_READ_GLOBALS | EFFECT_WRITE_PROPS | EFFECT_WRITE_GLOBALS;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Interner {
    map: IndexSet<String, BuildHasherDefault<FxHasher>>,
}

impl Default for Interner {
    fn default() -> Self {
        let mut interner = Interner {
            map: IndexSet::default(),
        };
        interner.intern("".to_string());
        interner.intern("this".to_string());
        interner.intern("<anonymous function>".to_string());
        interner.intern("isset".to_string());
        interner.intern("unset".to_string());
        interner.intern("HH\\MemberOf".to_string());
        interner.intern("echo".to_string());
        interner.intern("__construct".to_string());
        interner.intern("<data attribute>".to_string());
        interner.intern("<aria attribute>".to_string());
        interner.intern("HH\\AnyArray".to_string());
        interner.intern("HH\\KeyedContainer".to_string());
        interner.intern("HH\\Container".to_string());
        interner.intern("__PHP_Incomplete_Class".to_string());
        interner.intern("XHPChild".to_string());
        interner.intern("HH\\Awaitable".to_string());
        interner.intern("HH\\BuiltinEnum".to_string());
        interner.intern("HH\\BuiltinEnumClass".to_string());
        interner.intern("static".to_string());
        interner.intern("self".to_string());
        interner.intern("HH\\FormatString".to_string());
        interner.intern("HH\\EnumClass\\Label".to_string());
        interner.intern("HH\\Traversable".to_string());
        interner.intern("HH\\KeyedTraversable".to_string());
        interner.intern("HH\\Lib\\Regex\\Pattern".to_string());
        interner.intern("HH\\Iterator".to_string());
        interner.intern("HH\\KeyedIterator".to_string());
        interner.intern("HH\\AsyncIterator".to_string());
        interner.intern("HH\\AsyncKeyedIterator".to_string());
        interner.intern("HH\\Shapes".to_string());
        interner.intern("stdClass".to_string());
        interner.intern("SimpleXMLElement".to_string());
        interner.intern("HH\\Asio\\join".to_string());
        interner.intern("include".to_string());
        interner.intern("HH\\TypeStructure".to_string());
        interner.intern("HH\\invariant".to_string());
        interner.intern("coerce".to_string());
        interner.intern("assert".to_string());
        interner.intern("assertAll".to_string());
        interner.intern("keyExists".to_string());
        interner.intern("removeKey".to_string());
        interner.intern("HH\\Lib\\Math\\INT32_MAX".to_string());
        interner
    }
}

impl Interner {
    /// Get the id corresponding to `path`.
    ///
    /// If `path` does not exists in `self`, returns [`None`].
    pub fn get(&self, path: &str) -> Option<StrId> {
        self.map.get_index_of(path).map(|i| StrId(i as u32))
    }

    /// Insert `path` in `self`.
    ///
    /// - If `path` already exists in `self`, returns its associated id;
    /// - Else, returns a newly allocated id.
    pub fn intern(&mut self, path: String) -> StrId {
        let (id, _added) = self.map.insert_full(path);
        assert!(id < u32::MAX as usize);
        StrId(id as u32)
    }

    /// Returns the path corresponding to `id`.
    ///
    /// # Panics
    ///
    /// Panics if `id` does not exists in `self`.
    pub fn lookup(&self, id: &StrId) -> &str {
        self.map.get_index(id.0 as usize).unwrap()
    }

    pub fn get_map(&self) -> FxHashMap<String, StrId> {
        self.map
            .iter()
            .enumerate()
            .map(|(k, v)| (v.clone(), StrId(k as u32)))
            .collect()
    }
}

#[derive(Debug)]
pub struct ThreadedInterner {
    map: IndexMap<String, StrId>,
    reverse_map: BTreeMap<StrId, usize>,
    pub parent: Arc<Mutex<Interner>>,
}

impl ThreadedInterner {
    pub fn new(interner: Arc<Mutex<Interner>>) -> Self {
        ThreadedInterner {
            map: IndexMap::default(),
            reverse_map: BTreeMap::new(),
            parent: interner.clone(),
        }
    }

    pub fn intern_str(&mut self, path: &str) -> StrId {
        if let Some(id) = self.map.get(path) {
            return *id;
        }

        let id;
        {
            id = self.parent.lock().unwrap().intern(path.to_string());
        }
        let index = self.map.insert_full(path.to_string(), id).0;
        self.reverse_map.insert(id, index);

        id
    }

    pub fn intern(&mut self, path: String) -> StrId {
        if let Some(id) = self.map.get(&path) {
            return *id;
        }

        let id;
        {
            id = self.parent.lock().unwrap().intern(path.clone());
        }
        let index = self.map.insert_full(path, id).0;
        self.reverse_map.insert(id, index);

        id
    }

    pub fn lookup(&self, id: StrId) -> &str {
        if let Some(entry) = self.map.get_index(*self.reverse_map.get(&id).unwrap()) {
            entry.0
        } else {
            panic!()
        }
    }
}
