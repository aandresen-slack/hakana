use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::{
    diff::CodebaseDiff,
    function_context::{FunctionContext, FunctionLikeIdentifier},
    StrId, STR_EMPTY,
};

pub enum ReferenceSource {
    Symbol(bool, StrId),
    ClasslikeMember(bool, StrId, StrId),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolReferences {
    // A lookup table of all symbols (classes, functions, enums etc) that reference another symbol
    pub symbol_references_to_symbols: FxHashMap<(StrId, StrId), FxHashSet<(StrId, StrId)>>,

    // A lookup table of all symbols (classes, functions, enums etc) that reference another symbol
    pub symbol_references_to_symbols_in_signature:
        FxHashMap<(StrId, StrId), FxHashSet<(StrId, StrId)>>,

    // A lookup table of all symbols (classes, functions, enums etc) that reference a classlike member
    // (class method, enum case, class property etc)
    pub symbol_references_to_overridden_members:
        FxHashMap<(StrId, StrId), FxHashSet<(StrId, StrId)>>,

    // A lookup table used for getting all the functions that reference a method's return value
    // This is used for dead code detection when we want to see what return values are unused
    pub functionlike_references_to_functionlike_returns:
        FxHashMap<FunctionLikeIdentifier, FxHashSet<FunctionLikeIdentifier>>,
}

pub struct InvalidSymbols {
    pub invalid_symbol_and_member_signatures: FxHashSet<(StrId, StrId)>,
    pub invalid_symbol_and_member_bodies: FxHashSet<(StrId, StrId)>,
    pub partially_invalid_symbols: FxHashSet<StrId>,
}

impl SymbolReferences {
    pub fn new() -> Self {
        Self {
            symbol_references_to_symbols: FxHashMap::default(),
            symbol_references_to_symbols_in_signature: FxHashMap::default(),
            symbol_references_to_overridden_members: FxHashMap::default(),
            functionlike_references_to_functionlike_returns: FxHashMap::default(),
        }
    }

    pub fn add_symbol_reference_to_class_member(
        &mut self,
        referencing_symbol: StrId,
        class_member: (StrId, StrId),
        in_signature: bool,
    ) {
        self.add_symbol_reference_to_symbol(
            referencing_symbol.clone(),
            class_member.0.clone(),
            in_signature,
        );

        if in_signature {
            self.symbol_references_to_symbols_in_signature
                .entry((referencing_symbol, STR_EMPTY))
                .or_insert_with(FxHashSet::default)
                .insert(class_member);
        } else {
            self.symbol_references_to_symbols
                .entry((referencing_symbol, STR_EMPTY))
                .or_insert_with(FxHashSet::default)
                .insert(class_member);
        }
    }

    pub fn add_symbol_reference_to_symbol(
        &mut self,
        referencing_symbol: StrId,
        symbol: StrId,
        in_signature: bool,
    ) {
        if in_signature {
            self.symbol_references_to_symbols_in_signature
                .entry((referencing_symbol, STR_EMPTY))
                .or_insert_with(FxHashSet::default)
                .insert((symbol, STR_EMPTY));
        } else {
            self.symbol_references_to_symbols
                .entry((referencing_symbol, STR_EMPTY))
                .or_insert_with(FxHashSet::default)
                .insert((symbol, STR_EMPTY));
        }
    }

    pub fn add_class_member_reference_to_class_member(
        &mut self,
        referencing_class_member: (StrId, StrId),
        class_member: (StrId, StrId),
        in_signature: bool,
    ) {
        self.add_symbol_reference_to_symbol(
            referencing_class_member.0.clone(),
            class_member.0.clone(),
            in_signature,
        );

        self.add_class_member_reference_to_symbol(
            referencing_class_member.clone(),
            class_member.0.clone(),
            in_signature,
        );

        if in_signature {
            self.symbol_references_to_symbols_in_signature
                .entry(referencing_class_member)
                .or_insert_with(FxHashSet::default)
                .insert(class_member);
        } else {
            self.symbol_references_to_symbols
                .entry(referencing_class_member)
                .or_insert_with(FxHashSet::default)
                .insert(class_member);
        }
    }

    pub fn add_class_member_reference_to_symbol(
        &mut self,
        referencing_class_member: (StrId, StrId),
        symbol: StrId,
        in_signature: bool,
    ) {
        self.add_symbol_reference_to_symbol(
            referencing_class_member.0.clone(),
            symbol.clone(),
            in_signature,
        );

        if in_signature {
            self.symbol_references_to_symbols_in_signature
                .entry(referencing_class_member)
                .or_insert_with(FxHashSet::default)
                .insert((symbol, STR_EMPTY));
        } else {
            self.symbol_references_to_symbols
                .entry(referencing_class_member)
                .or_insert_with(FxHashSet::default)
                .insert((symbol, STR_EMPTY));
        }
    }

    pub fn add_reference_to_class_member(
        &mut self,
        function_context: &FunctionContext,
        class_member: (StrId, StrId),
        in_signature: bool,
    ) {
        if let Some(referencing_functionlike) = &function_context.calling_functionlike_id {
            match referencing_functionlike {
                FunctionLikeIdentifier::Function(function_name) => self
                    .add_symbol_reference_to_class_member(
                        function_name.clone(),
                        class_member,
                        in_signature,
                    ),
                FunctionLikeIdentifier::Method(class_name, function_name) => self
                    .add_class_member_reference_to_class_member(
                        (class_name.clone(), function_name.clone()),
                        class_member,
                        in_signature,
                    ),
            }
        } else if let Some(calling_class) = &function_context.calling_class {
            self.add_symbol_reference_to_class_member(
                calling_class.clone(),
                class_member,
                in_signature,
            )
        }
    }

    pub fn add_reference_to_overridden_class_member(
        &mut self,
        function_context: &FunctionContext,
        class_member: (StrId, StrId),
    ) {
        if let Some(referencing_functionlike) = &function_context.calling_functionlike_id {
            match referencing_functionlike {
                FunctionLikeIdentifier::Function(function_name) => {
                    self.symbol_references_to_overridden_members
                        .entry((*function_name, STR_EMPTY))
                        .or_insert_with(FxHashSet::default)
                        .insert(class_member);
                }
                FunctionLikeIdentifier::Method(class_name, function_name) => {
                    self.symbol_references_to_overridden_members
                        .entry((class_name.clone(), function_name.clone()))
                        .or_insert_with(FxHashSet::default)
                        .insert(class_member);
                }
            }
        } else if let Some(calling_class) = &function_context.calling_class {
            self.symbol_references_to_overridden_members
                .entry((*calling_class, STR_EMPTY))
                .or_insert_with(FxHashSet::default)
                .insert(class_member);
        }
    }

    pub fn add_reference_to_symbol(
        &mut self,
        function_context: &FunctionContext,
        symbol: StrId,
        in_signature: bool,
    ) {
        if let Some(referencing_functionlike) = &function_context.calling_functionlike_id {
            match referencing_functionlike {
                FunctionLikeIdentifier::Function(function_name) => {
                    self.add_symbol_reference_to_symbol(function_name.clone(), symbol, in_signature)
                }
                FunctionLikeIdentifier::Method(class_name, function_name) => self
                    .add_class_member_reference_to_symbol(
                        (class_name.clone(), function_name.clone()),
                        symbol,
                        in_signature,
                    ),
            }
        } else if let Some(calling_class) = &function_context.calling_class {
            self.add_symbol_reference_to_symbol(calling_class.clone(), symbol, in_signature)
        }
    }

    pub fn add_reference_to_functionlike_return(
        &mut self,
        referencing_functionlike: FunctionLikeIdentifier,
        functionlike: FunctionLikeIdentifier,
    ) {
        self.functionlike_references_to_functionlike_returns
            .entry(referencing_functionlike)
            .or_insert_with(FxHashSet::default)
            .insert(functionlike);
    }

    pub fn extend(&mut self, other: Self) {
        for (k, v) in other.symbol_references_to_symbols {
            self.symbol_references_to_symbols
                .entry(k)
                .or_insert_with(FxHashSet::default)
                .extend(v);
        }

        for (k, v) in other.symbol_references_to_symbols_in_signature {
            self.symbol_references_to_symbols_in_signature
                .entry(k)
                .or_insert_with(FxHashSet::default)
                .extend(v);
        }

        for (k, v) in other.symbol_references_to_overridden_members {
            self.symbol_references_to_overridden_members
                .entry(k)
                .or_insert_with(FxHashSet::default)
                .extend(v);
        }
    }

    pub fn get_referenced_symbols_and_members(&self) -> FxHashSet<&(StrId, StrId)> {
        let mut referenced_symbols_and_members = FxHashSet::default();

        for (_, symbol_references_to_symbols) in &self.symbol_references_to_symbols {
            referenced_symbols_and_members.extend(symbol_references_to_symbols);
        }

        for (_, symbol_references_to_symbols) in &self.symbol_references_to_symbols_in_signature {
            referenced_symbols_and_members.extend(symbol_references_to_symbols);
        }

        referenced_symbols_and_members
    }

    pub fn get_referenced_symbols_and_members_with_counts(&self) -> FxHashMap<(StrId, StrId), u32> {
        let mut referenced_symbols_and_members = FxHashMap::default();

        for (_, symbol_references_to_symbols) in &self.symbol_references_to_symbols {
            for r in symbol_references_to_symbols {
                let v = referenced_symbols_and_members.entry(*r).or_insert(0);
                *v += 1;
            }
        }

        for (_, symbol_references_to_symbols) in &self.symbol_references_to_symbols_in_signature {
            for r in symbol_references_to_symbols {
                let v = referenced_symbols_and_members.entry(*r).or_insert(0);
                *v += 1;
            }
        }

        referenced_symbols_and_members
    }

    pub fn get_referenced_overridden_class_members(&self) -> FxHashSet<&(StrId, StrId)> {
        let mut referenced_class_members = FxHashSet::default();

        for (_, symbol_references_to_class_members) in &self.symbol_references_to_overridden_members
        {
            referenced_class_members.extend(symbol_references_to_class_members);
        }

        referenced_class_members
    }

    pub fn get_invalid_symbols(&self, codebase_diff: &CodebaseDiff) -> Option<InvalidSymbols> {
        let mut invalid_symbols = FxHashSet::default();
        let mut invalid_symbol_members = FxHashSet::default();

        let mut new_invalid_symbols = codebase_diff
            .add_or_delete
            .iter()
            .map(|v| *v)
            .collect::<Vec<_>>();

        let mut seen_symbols = FxHashSet::default();

        let mut expense = 0;

        while !new_invalid_symbols.is_empty() {
            if expense > 5000 {
                return None;
            }

            let new_invalid_symbol = new_invalid_symbols.pop().unwrap();

            if seen_symbols.contains(&new_invalid_symbol) {
                continue;
            }

            invalid_symbols.insert(new_invalid_symbol);

            seen_symbols.insert(new_invalid_symbol);

            for (referencing_member, referenced_members) in
                &self.symbol_references_to_symbols_in_signature
            {
                if referenced_members.contains(&new_invalid_symbol) {
                    new_invalid_symbols.push(*referencing_member);
                    if !referencing_member.1.is_empty() {
                        invalid_symbol_members.insert(*referencing_member);
                    } else {
                        invalid_symbols.insert(*referencing_member);
                    }
                    expense += 1;
                }
            }

            expense += 1;

            if !new_invalid_symbol.1.is_empty() {
                invalid_symbol_members.insert(new_invalid_symbol);
            } else {
                invalid_symbols.insert((new_invalid_symbol.0, STR_EMPTY));
            }
        }

        let mut invalid_symbol_bodies = FxHashSet::default();
        let mut invalid_symbol_member_bodies = FxHashSet::default();

        for invalid_symbol_member in &invalid_symbols {
            for (referencing_member, referenced_members) in &self.symbol_references_to_symbols {
                if referenced_members.contains(&(invalid_symbol_member.0, invalid_symbol_member.1))
                {
                    if invalid_symbol_member.1.is_empty() {
                        invalid_symbol_bodies.insert(*referencing_member);
                    } else {
                        invalid_symbol_member_bodies.insert(*referencing_member);
                    }
                }
            }
        }

        for invalid_symbol_member in &invalid_symbol_members {
            for (referencing_member, referenced_members) in &self.symbol_references_to_symbols {
                if referenced_members.contains(&(invalid_symbol_member.0, invalid_symbol_member.1))
                {
                    if invalid_symbol_member.1.is_empty() {
                        invalid_symbol_bodies.insert(*referencing_member);
                    } else {
                        invalid_symbol_member_bodies.insert(*referencing_member);
                    }
                }
            }
        }

        for keep_signature in &codebase_diff.keep_signature {
            if !keep_signature.1.is_empty() {
                invalid_symbol_member_bodies.insert((keep_signature.0, keep_signature.1));
            } else {
                invalid_symbol_bodies.insert(*keep_signature);
            }
        }

        let mut partially_invalid_symbols = invalid_symbol_members
            .iter()
            .map(|(a, _)| *a)
            .collect::<FxHashSet<_>>();

        partially_invalid_symbols.extend(invalid_symbol_member_bodies.iter().map(|(a, _)| *a));

        invalid_symbols.extend(invalid_symbol_members);

        invalid_symbol_bodies.extend(invalid_symbol_member_bodies);

        Some(InvalidSymbols {
            invalid_symbol_and_member_signatures: invalid_symbols,
            invalid_symbol_and_member_bodies: invalid_symbol_bodies,
            partially_invalid_symbols,
        })
    }

    pub fn remove_references_from_invalid_symbols(
        &mut self,
        invalid_symbols_and_members: &FxHashSet<(StrId, StrId)>,
    ) {
        self.symbol_references_to_symbols
            .retain(|symbol, _| !invalid_symbols_and_members.contains(symbol));
        self.symbol_references_to_symbols_in_signature
            .retain(|symbol, _| !invalid_symbols_and_members.contains(symbol));
    }
}
