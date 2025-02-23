use crate::cycle::CycleRecoveryStrategy;
use crate::ingredient::{fmt_index, Ingredient, IngredientRequiresReset};
use crate::key::DependencyIndex;
use crate::runtime::local_state::QueryOrigin;
use crate::runtime::StampedValue;
use crate::{AsId, DatabaseKeyIndex, Durability, Id, IngredientIndex, Revision, Runtime};
use rustc_hash::FxHashMap;
use std::fmt;
use std::hash::Hash;

/// Ingredient used to represent the fields of a `#[salsa::input]`.
/// These fields can only be mutated by an explicit call to a setter
/// with an `&mut` reference to the database,
/// and therefore cannot be mutated during a tracked function or in parallel.
/// This makes the implementation considerably simpler.
pub struct InputFieldIngredient<K, F> {
    index: IngredientIndex,
    map: FxHashMap<K, StampedValue<F>>,
    debug_name: &'static str,
}

impl<K, F> InputFieldIngredient<K, F>
where
    K: Eq + Hash + AsId,
{
    pub fn new(index: IngredientIndex, debug_name: &'static str) -> Self {
        Self {
            index,
            map: Default::default(),
            debug_name,
        }
    }

    pub fn store(
        &mut self,
        runtime: &mut Runtime,
        key: K,
        value: F,
        durability: Durability,
    ) -> Option<F> {
        let revision = runtime.current_revision();
        let stamped_value = StampedValue {
            value,
            durability,
            changed_at: revision,
        };

        if let Some(old_value) = self.map.insert(key, stamped_value) {
            Some(old_value.value)
        } else {
            None
        }
    }

    pub fn fetch(&self, runtime: &Runtime, key: K) -> &F {
        let StampedValue {
            value,
            durability,
            changed_at,
        } = self.map.get(&key).unwrap();

        runtime.report_tracked_read(
            self.database_key_index(key).into(),
            *durability,
            *changed_at,
        );

        value
    }

    fn database_key_index(&self, key: K) -> DatabaseKeyIndex {
        DatabaseKeyIndex {
            ingredient_index: self.index,
            key_index: key.as_id(),
        }
    }
}

impl<DB: ?Sized, K, F> Ingredient<DB> for InputFieldIngredient<K, F>
where
    K: AsId,
{
    fn cycle_recovery_strategy(&self) -> CycleRecoveryStrategy {
        CycleRecoveryStrategy::Panic
    }

    fn maybe_changed_after(&self, _db: &DB, input: DependencyIndex, revision: Revision) -> bool {
        let key = K::from_id(input.key_index.unwrap());
        self.map.get(&key).unwrap().changed_at > revision
    }

    fn origin(&self, _key_index: Id) -> Option<QueryOrigin> {
        None
    }

    fn mark_validated_output(
        &self,
        _db: &DB,
        _executor: DatabaseKeyIndex,
        _output_key: Option<Id>,
    ) {
    }

    fn remove_stale_output(
        &self,
        _db: &DB,
        _executor: DatabaseKeyIndex,
        _stale_output_key: Option<Id>,
    ) {
    }

    fn salsa_struct_deleted(&self, _db: &DB, _id: Id) {
        panic!("unexpected call: input fields are never deleted");
    }

    fn reset_for_new_revision(&mut self) {
        panic!("unexpected call: input fields don't register for resets");
    }

    fn fmt_index(&self, index: Option<crate::Id>, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_index(self.debug_name, index, fmt)
    }
}

impl<K, F> IngredientRequiresReset for InputFieldIngredient<K, F>
where
    K: AsId,
{
    const RESET_ON_NEW_REVISION: bool = false;
}
