pub mod clause;

pub use clause::Clause;
use hakana_reflection_info::assertion::Assertion;
use rand::Rng;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
mod tests;

pub fn negate_types(
    all_types: BTreeMap<String, Vec<Vec<Assertion>>>,
) -> BTreeMap<String, Vec<Vec<Assertion>>> {
    let mut new_all_types: BTreeMap<String, Vec<Vec<Assertion>>> = BTreeMap::new();

    'outer: for (var, anded_types) in all_types.iter() {
        if anded_types.len() > 1 {
            let mut new_anded_types: Vec<Assertion> = Vec::new();

            for orred_types in anded_types.iter() {
                if orred_types.len() > 1 {
                    break 'outer;
                }

                new_anded_types.push(orred_types[0].get_negation());
            }

            new_all_types.insert(var.to_string(), vec![new_anded_types]);
            break;
        }

        let mut new_orred_types: Vec<Vec<Assertion>> = Vec::new();

        for orred_type in anded_types[0].iter() {
            new_orred_types.push(vec![orred_type.get_negation()]);
        }

        new_all_types.insert(var.to_string(), new_orred_types);
    }

    return new_all_types;
}

fn keys_match<T: Eq + Ord, U, V>(map1: &BTreeMap<T, U>, map2: &BTreeMap<T, V>) -> bool {
    map1.len() == map2.len() && map1.keys().all(|k| map2.contains_key(k))
}

// This is a very simple simplification heuristic
// for CNF formulae.
//
// It simplifies formulae:
//     ($a) && ($a || $b) => $a
//     (!$a) && (!$b) && ($a || $b || $c) => $c
pub fn simplify_cnf(clauses: Vec<&Clause>) -> Vec<Clause> {
    let clause_count = clauses.len();

    if clause_count > 50 {
        let mut all_has_unknown = true;

        for clause in clauses.iter() {
            let mut clause_has_unknown = false;

            for (key, _) in clause.possibilities.iter() {
                if &key[0..1] == "*" {
                    clause_has_unknown = true;
                    break;
                }
            }

            if !clause_has_unknown {
                all_has_unknown = false;
                break;
            }
        }

        if all_has_unknown {
            return clauses.into_iter().map(|v| v.clone()).collect();
        }
    }

    let mut unique_clauses = clauses.into_iter().collect::<HashSet<_>>();

    let mut removed_clauses = HashSet::new();
    let mut added_clauses = HashSet::new();

    // remove impossible types
    'outer: for clause_a in &unique_clauses {
        if !clause_a.reconcilable || clause_a.wedge {
            continue;
        }

        let mut is_clause_a_simple: bool = true;

        if clause_a.possibilities.len() != 1 {
            is_clause_a_simple = false;
        } else {
            for (_, var_possibilities) in &clause_a.possibilities {
                if var_possibilities.len() != 1 {
                    is_clause_a_simple = false;
                }
            }
        }

        if !is_clause_a_simple {
            'inner: for clause_b in &unique_clauses {
                if clause_a == clause_b || !clause_b.reconcilable || clause_b.wedge {
                    continue;
                }

                if keys_match(&clause_a.possibilities, &clause_b.possibilities) {
                    let mut opposing_keys = vec![];

                    for (key, a_possibilities) in clause_a.possibilities.iter() {
                        let b_possibilities = &clause_b.possibilities[key];
                        if keys_match(&a_possibilities, &b_possibilities) {
                            continue;
                        }

                        if a_possibilities.len() == 1
                            && b_possibilities.len() == 1
                            && a_possibilities
                                .values()
                                .next()
                                .unwrap()
                                .is_negation_of(&b_possibilities.values().next().unwrap())
                        {
                            opposing_keys.push(key.clone());
                            continue;
                        }

                        continue 'inner;
                    }

                    if opposing_keys.len() == 1 {
                        removed_clauses.insert(clause_a.clone());

                        let maybe_new_clause = clause_a.remove_possibilities(&opposing_keys[0]);

                        if maybe_new_clause == None {
                            continue 'outer;
                        }

                        added_clauses.insert(maybe_new_clause.unwrap());
                    }
                }
            }

            continue;
        }

        // only iterates over one single possibility
        for (clause_var, var_possibilities) in &clause_a.possibilities {
            let only_type = &var_possibilities.values().next().unwrap();
            let negated_clause_type = only_type.get_negation();
            let negated_string = negated_clause_type.to_string();

            for clause_b in &unique_clauses {
                if clause_a == clause_b || !clause_b.reconcilable || clause_b.wedge {
                    continue;
                }

                if let Some(matching_clause_possibilities) = clause_b.possibilities.get(clause_var)
                {
                    if matching_clause_possibilities.contains_key(&negated_string) {
                        let mut clause_var_possibilities = matching_clause_possibilities.clone();

                        clause_var_possibilities.retain(|k, _| k != &negated_string);

                        removed_clauses.insert(clause_b.clone());

                        if clause_var_possibilities.len() == 0 {
                            let maybe_updated_clause = clause_b.remove_possibilities(&clause_var);

                            if let Some(x) = maybe_updated_clause {
                                added_clauses.insert(x);
                            }
                        } else {
                            let updated_clause = clause_b
                                .add_possibility(clause_var.clone(), clause_var_possibilities);

                            added_clauses.insert(updated_clause);
                        }
                    }
                }
            }
        }
    }

    unique_clauses.retain(|f| !removed_clauses.contains(f));

    let mut unique_clauses = unique_clauses
        .into_iter()
        .map(|c| c.clone())
        .collect::<HashSet<_>>();
    unique_clauses.extend(added_clauses);

    let mut simplified_clauses = HashSet::new();

    for clause_a in &unique_clauses {
        let mut is_redundant = false;

        for clause_b in &unique_clauses {
            if clause_a == clause_b || !clause_b.reconcilable || clause_b.wedge || clause_a.wedge {
                continue;
            }

            if clause_a.contains(clause_b) {
                is_redundant = true;
                break;
            }
        }

        if !is_redundant {
            simplified_clauses.insert(clause_a.clone());
        }
    }

    // simplify (A || X) && (!A || Y) && (X || Y)
    // to
    // simplify (A || X) && (!A || Y)
    // where X and Y are sets of orred terms
    if simplified_clauses.len() > 2 && simplified_clauses.len() < 256 {
        let mut compared_clauses = HashSet::new();

        let mut removed_clauses = HashSet::new();

        for clause_a in &simplified_clauses {
            for clause_b in &simplified_clauses {
                if clause_a == clause_b
                    || compared_clauses.contains(&(clause_b.hash, clause_a.hash))
                {
                    continue;
                }

                compared_clauses.insert((clause_a.hash, clause_b.hash));

                let common_keys = clause_a
                    .possibilities
                    .iter()
                    .filter(|(var_id, _)| clause_b.possibilities.contains_key(*var_id))
                    .map(|(var_id, _)| var_id)
                    .collect::<HashSet<_>>();

                if !common_keys.is_empty() {
                    let mut common_negated_keys = HashSet::new();

                    for common_key in common_keys {
                        let clause_a_possibilities =
                            clause_a.possibilities.get(common_key).unwrap();
                        let clause_b_possibilities =
                            clause_b.possibilities.get(common_key).unwrap();
                        if clause_a_possibilities.len() == 1
                            && clause_b_possibilities.len() == 1
                            && clause_a_possibilities
                                .values()
                                .next()
                                .unwrap()
                                .is_negation_of(clause_b_possibilities.values().next().unwrap())
                        {
                            common_negated_keys.insert(common_key);
                        }
                    }

                    if !common_negated_keys.is_empty() {
                        let mut new_possibilities = BTreeMap::new();

                        for (var_id, possibilities) in &clause_a.possibilities {
                            if common_negated_keys.contains(var_id) {
                                continue;
                            }

                            new_possibilities
                                .entry(var_id.clone())
                                .or_insert_with(BTreeMap::new)
                                .extend(possibilities.clone());
                        }

                        for (var_id, possibilities) in &clause_b.possibilities {
                            if common_negated_keys.contains(var_id) {
                                continue;
                            }

                            new_possibilities
                                .entry(var_id.clone())
                                .or_insert_with(BTreeMap::new)
                                .extend(possibilities.clone());
                        }

                        let conflict_clause = Clause::new(
                            new_possibilities,
                            clause_a.creating_conditional_id,
                            clause_a.creating_object_id,
                            None,
                            None,
                            None,
                            None,
                        );

                        removed_clauses.insert(conflict_clause);
                    }
                }
            }
        }

        simplified_clauses.retain(|f| !removed_clauses.contains(f));
    }

    return simplified_clauses.into_iter().collect::<Vec<_>>();
}

pub fn get_truths_from_formula(
    clauses: Vec<&Clause>,
    creating_conditional_id: Option<(usize, usize)>,
    cond_referenced_var_ids: &mut HashSet<String>,
) -> (
    BTreeMap<String, Vec<Vec<Assertion>>>,
    BTreeMap<String, HashMap<usize, Vec<Assertion>>>,
) {
    let mut truths = BTreeMap::new();

    let mut active_truths = BTreeMap::new();

    for clause in clauses {
        if !clause.reconcilable || clause.possibilities.len() != 1 {
            continue;
        }

        for (var_id, possible_types) in &clause.possibilities {
            if var_id.starts_with("*") {
                continue;
            }

            if possible_types.len() == 1 {
                let possible_type = possible_types.values().next().unwrap();

                let redeffed_vars_contains = if let Some(redefined_vars) = &clause.redefined_vars {
                    redefined_vars.contains(var_id)
                } else {
                    false
                };

                if !redeffed_vars_contains {
                    truths
                        .entry(var_id.clone())
                        .or_insert_with(Vec::new)
                        .push(vec![possible_type.clone()]);
                } else {
                    truths.insert(var_id.clone(), vec![vec![possible_type.clone()]]);
                }

                if let Some(creating_conditional_id) = creating_conditional_id {
                    if creating_conditional_id == clause.creating_conditional_id {
                        active_truths
                            .entry(var_id.clone())
                            .or_insert_with(HashMap::new)
                            .insert(
                                truths.get(var_id).unwrap().len() - 1,
                                vec![possible_type.clone()],
                            );
                    }
                }
            } else {
                let mut things_that_can_be_said = HashMap::new();

                for (_, assertion) in possible_types {
                    if matches!(assertion, Assertion::Falsy) || !assertion.has_negation() {
                        things_that_can_be_said.insert(assertion.to_string(), assertion);
                    }
                }

                if !things_that_can_be_said.is_empty()
                    && things_that_can_be_said.len() == possible_types.len()
                {
                    if clause.generated {
                        cond_referenced_var_ids.remove(var_id);
                    }

                    let things_vec = things_that_can_be_said
                        .into_iter()
                        .map(|(_, v)| v.clone())
                        .collect::<Vec<Assertion>>();

                    truths.insert(var_id.clone(), vec![things_vec.clone()]);

                    if let Some(creating_conditional_id) = creating_conditional_id {
                        if creating_conditional_id == clause.creating_conditional_id {
                            active_truths
                                .entry(var_id.clone())
                                .or_insert_with(HashMap::new)
                                .insert(truths.get(var_id).unwrap().len() - 1, things_vec.clone());
                        }
                    }
                }
            }
        }
    }

    (truths, active_truths)
}

fn group_impossibilities(mut clauses: Vec<Clause>) -> Result<Vec<Clause>, String> {
    let mut complexity = 1;

    let mut seed_clauses = vec![];

    let clause = clauses.pop();

    if clause == None {
        panic!("there should be clauses")
    }

    let clause = clause.unwrap();

    if !clause.wedge {
        let impossibilities = clause.get_impossibilities();

        for (var, impossible_types) in impossibilities.iter() {
            for impossible_type in impossible_types.iter() {
                let mut seed_clause_possibilities = BTreeMap::new();
                seed_clause_possibilities.insert(
                    var.clone(),
                    BTreeMap::from([(impossible_type.to_string(), impossible_type.clone())]),
                );

                let seed_clause = Clause::new(
                    seed_clause_possibilities,
                    clause.creating_conditional_id,
                    clause.creating_object_id,
                    None,
                    None,
                    None,
                    None,
                );

                seed_clauses.push(seed_clause);

                complexity += 1;
            }
        }
    }

    if clauses.len() == 0 || seed_clauses.len() == 0 {
        return Ok(seed_clauses);
    }

    let mut upper_bound = seed_clauses.len();

    for c in &clauses {
        let mut i = 0;
        for (_, p) in &c.possibilities {
            i += p.len();
        }
        upper_bound *= i;
        if upper_bound > 20000 {
            return Err("Complicated".to_string());
        }
    }

    while clauses.len() > 0 {
        let clause = clauses.pop().unwrap();

        let mut new_clauses = vec![];

        for grouped_clause in &seed_clauses {
            let clause_impossibilities = clause.get_impossibilities();

            for (var, impossible_types) in clause_impossibilities {
                'next: for impossible_type in impossible_types {
                    if let Some(new_insert_value) = grouped_clause.possibilities.get(&var) {
                        for (_, a) in new_insert_value {
                            if a.is_negation_of(&impossible_type) {
                                break 'next;
                            }
                        }
                    }

                    let mut new_clause_possibilities = grouped_clause.possibilities.clone();

                    new_clause_possibilities
                        .entry(var.clone())
                        .or_insert_with(BTreeMap::new)
                        .insert(impossible_type.to_string(), impossible_type);

                    new_clauses.push(Clause::new(
                        new_clause_possibilities,
                        grouped_clause.creating_conditional_id,
                        clause.creating_object_id,
                        Some(false),
                        Some(true),
                        Some(true),
                        None,
                    ));

                    complexity += 1;

                    if complexity > 20000 {
                        return Err("Complicated".to_string());
                    }
                }
            }
        }

        seed_clauses = new_clauses;
    }

    return Ok(seed_clauses);
}

pub fn combine_ored_clauses(
    left_clauses: &Vec<Clause>,
    right_clauses: &Vec<Clause>,
    conditional_object_id: (usize, usize),
) -> Result<Vec<Clause>, String> {
    let mut clauses = vec![];

    let mut all_wedges = true;
    let mut has_wedge = false;

    let upper_bound_output = left_clauses.len() * right_clauses.len();

    if upper_bound_output > 2048 {
        return Err("too many clauses".to_string());
    }

    if left_clauses.is_empty() || right_clauses.is_empty() {
        return Ok(vec![]);
    }

    for left_clause in left_clauses {
        for right_clause in right_clauses {
            all_wedges = all_wedges && (left_clause.wedge && right_clause.wedge);
            has_wedge = has_wedge || (left_clause.wedge && right_clause.wedge);
        }
    }

    if all_wedges {
        return Ok(vec![Clause::new(
            BTreeMap::new(),
            conditional_object_id,
            conditional_object_id,
            Some(true),
            None,
            None,
            None,
        )]);
    }

    for left_clause in left_clauses {
        'right: for right_clause in right_clauses {
            if left_clause.wedge && right_clause.wedge {
                // handled below
                continue;
            }

            let mut possibilities = BTreeMap::new();

            let can_reconcile = !left_clause.wedge
                && !right_clause.wedge
                && left_clause.reconcilable
                && right_clause.reconcilable;

            for (var, possible_types) in &left_clause.possibilities {
                possibilities
                    .entry(var.clone())
                    .or_insert_with(BTreeMap::new)
                    .extend(possible_types.clone());
            }

            for (var, possible_types) in &right_clause.possibilities {
                possibilities
                    .entry(var.clone())
                    .or_insert_with(BTreeMap::new)
                    .extend(possible_types.clone());
            }

            for (_, var_possibilities) in &possibilities {
                if var_possibilities.len() == 2 {
                    let vals = var_possibilities.values().collect::<Vec<_>>();
                    if vals[0].is_negation_of(&vals[1]) {
                        continue 'right;
                    }
                }
            }

            let creating_conditional_id;

            if right_clause.creating_conditional_id == left_clause.creating_conditional_id {
                creating_conditional_id = right_clause.creating_conditional_id;
            } else {
                creating_conditional_id = conditional_object_id;
            }

            let is_generated = right_clause.generated
                || left_clause.generated
                || left_clauses.len() > 1
                || right_clauses.len() > 1;

            clauses.push(Clause::new(
                possibilities,
                creating_conditional_id,
                creating_conditional_id,
                Some(false),
                Some(can_reconcile),
                Some(is_generated),
                None,
            ))
        }
    }

    if has_wedge {
        clauses.push(Clause::new(
            BTreeMap::new(),
            conditional_object_id,
            conditional_object_id,
            Some(true),
            None,
            None,
            None,
        ));
    }

    return Ok(clauses);
}

// Negates a set of clauses
// negateClauses([$a || $b]) => !$a && !$b
// negateClauses([$a, $b]) => !$a || !$b
// negateClauses([$a, $b || $c]) =>
//   (!$a || !$b) &&
//   (!$a || !$c)
// negateClauses([$a, $b || $c, $d || $e || $f]) =>
//   (!$a || !$b || !$d) &&
//   (!$a || !$b || !$e) &&
//   (!$a || !$b || !$f) &&
//   (!$a || !$c || !$d) &&
//   (!$a || !$c || !$e) &&
//   (!$a || !$c || !$f)
pub fn negate_formula(mut clauses: Vec<Clause>) -> Result<Vec<Clause>, String> {
    clauses.retain(|clause| clause.reconcilable);

    if clauses.len() == 0 {
        let mut rng = rand::thread_rng();

        let n2: usize = rng.gen();
        return Ok(vec![Clause::new(
            BTreeMap::new(),
            (n2, n2),
            (n2, n2),
            Some(true),
            None,
            None,
            None,
        )]);
    }

    let impossible_clauses = group_impossibilities(clauses);

    if let Err(x) = impossible_clauses {
        return Err(x);
    }

    let impossible_clauses = impossible_clauses.unwrap();

    if impossible_clauses.len() == 0 {
        let mut rng = rand::thread_rng();

        let n2: usize = rng.gen();
        return Ok(vec![Clause::new(
            BTreeMap::new(),
            (n2, n2),
            (n2, n2),
            Some(true),
            None,
            None,
            None,
        )]);
    }

    let negated = simplify_cnf(impossible_clauses.iter().collect());

    if negated.len() == 0 {
        let mut rng = rand::thread_rng();

        let n2: usize = rng.gen();
        return Ok(vec![Clause::new(
            BTreeMap::new(),
            (n2, n2),
            (n2, n2),
            Some(true),
            None,
            None,
            None,
        )]);
    }

    return Ok(negated);
}
