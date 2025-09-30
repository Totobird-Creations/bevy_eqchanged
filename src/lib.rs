#![doc = include_str!("../README.md")]


use core::marker::PhantomData;
#[cfg(not(feature = "intmap"))]
use std::collections::{
    BTreeMap as HandledMap,
    btree_map::Entry as HandledMapEntry
};
use bevy_ecs::{
    archetype::Archetype,
    component::{
        Component,
        ComponentId,
        Components,
        Tick
    },
    entity::Entity,
    query::{
        QueryFilter,
        QueryData,
        WorldQuery,
        FilteredAccess,
        Changed
    },
    resource::Resource,
    storage::{ Table, TableRow },
    world::{
        World,
        unsafe_world_cell::UnsafeWorldCell,
        DeferredWorld,
        EntityWorldMut
    }
};
#[cfg(feature = "intmap")]
use intmap::{
    IntMap as HandledMap,
    Entry as HandledMapEntry
};


#[derive(Resource, Default)]
struct NextQueryId(usize);

#[derive(Component)]
struct PreviousValue<T> {
    previous : T,
    changed  : Tick,
    handled  : HandledMap<usize, Tick>
}


/// A filter on a component that only retains results the first time after they have been added or
///  modified in a way where the new value [`!=`](PartialEq::ne) the previous.
///
/// A common use for this filter is avoiding redundant work when values have not changed.
///
/// **Note** that unlike [`Changed`], *mutably dereferencing* a component
///  is not enough to be considered a change. The new value must [`!=`](PartialEq::ne) the previous.
///
/// ### Deferred
/// Note, that entity modifications issies with [`Commands`](bevy_ecs::system::Commands) (like entity
///  creation or entity component addition or removal) are visible only after deferred operations are
///  applied, typically at the end of the schedule iteration.
///
/// ### Time complexity
/// `EqChanged` is not [`ArchetypeFilter`](bevy_ecs::query::ArchetypeFilter), which practically means
///  that if query (with `T` component filter) matches million entities, `EqChanged<T>` filter will
///  iterate over all of them even if none of them were changed.
///
/// In order to track changes, `EqChanged` will [`Clone`] values when a modification has been detected.
///  Usually, the value will only be cloned once per change, but some exceptions occur if an entity
///  was spawned before the first time a system runs.
///
/// ### Examples
/// ```rust
/// fn print_moving_objects_system(query: Query<&Name, EqChanged<Transform>>) {
///     for name in &query {
///         println!("Entity moved: {:?}", name);
///     }
/// }
/// ```
pub struct EqChanged<T>(PhantomData<T>);


mod private {
    use super::*;

    pub struct EqChangedState<T>
    where
        T : Component
    {
        pub(crate) query_id : usize,
        pub(crate) changed  : <Changed<T> as WorldQuery>::State,
        pub(crate) old      : <Option<&'static mut PreviousValue<T>> as WorldQuery>::State,
        pub(crate) new      : <&'static T as WorldQuery>::State
    }

    pub struct EqChangedFetch<'l, T>
    where
        T : Component
    {
        pub(crate) world    : DeferredWorld<'l>,
        pub(crate) query_id : usize,
        pub(crate) this_run : Tick,
        pub(crate) changed  : <Changed<T> as WorldQuery>::Fetch<'l>,
        pub(crate) old      : <Option<&'static mut PreviousValue<T>> as WorldQuery>::Fetch<'l>,
        pub(crate) new      : <&'static T as WorldQuery>::Fetch<'l>
    }

    impl<'l, T> Clone for EqChangedFetch<'l, T>
    where
        T : Component
    {
        fn clone(&self) -> Self { unreachable!() }
    }

}
use private::*;


unsafe impl<T> QueryFilter for EqChanged<T>
where
    T : Component + PartialEq + Clone
{
    const IS_ARCHETYPAL : bool = false;

    unsafe fn filter_fetch(
        fetch     : &mut Self::Fetch<'_>,
        entity    : Entity,
        table_row : TableRow,
    ) -> bool { unsafe {
        if (! <Changed<T> as QueryFilter>::filter_fetch(&mut fetch.changed, entity, table_row)) {
            return false;
        }
        let old = <Option<&mut PreviousValue<T>> as QueryData>::fetch(&mut fetch.old, entity, table_row);
        let new = <&T as QueryData>::fetch(&mut fetch.new, entity, table_row);
        match (old) {
            // PreviousValue<T> was not in the current query.
            None => {
                let query_id = fetch.query_id;
                let this_run = fetch.this_run;
                let new      = new.clone();
                fetch.world.commands().entity(entity).queue(move |mut world : EntityWorldMut| {
                    match (world.get_mut::<PreviousValue<T>>()) {
                        // PreviousValue<T> does actually exist. It was just added in the same tick.
                        Some(mut previous_value) => {
                            if (new != previous_value.previous) {
                                previous_value.previous = new;
                                previous_value.changed  = this_run;
                            }
                            let changed = previous_value.changed;
                            match (previous_value.handled.entry(query_id)) {
                                HandledMapEntry::Occupied(mut entry) => {
                                    if (entry.get() != &changed) {
                                        entry.insert(changed);
                                    }
                                },
                                HandledMapEntry::Vacant(entry) => {
                                    entry.insert(changed);
                                }
                            }
                        },
                        // PreviousValue<T> does not exist. Add it.
                        None => {
                            let mut previous_value = PreviousValue {
                                previous : new,
                                changed  : this_run,
                                handled  : HandledMap::new()
                            };
                            previous_value.handled.insert(query_id, this_run);
                            world.insert(previous_value);
                        }
                    }
                });
                true
            },
            // PreviousValue<T> was in the current query.
            Some(mut previous_value) => {
                if (new != &previous_value.previous) {
                    previous_value.previous = new.clone();
                    previous_value.changed  = fetch.this_run;
                }
                let changed = previous_value.changed;
                match (previous_value.handled.entry(fetch.query_id)) {
                    HandledMapEntry::Occupied(mut entry) => {
                        if (entry.get() != &changed) {
                            entry.insert(changed);
                            true
                        } else { false }
                    },
                    HandledMapEntry::Vacant(entry) => {
                        entry.insert(changed);
                        true
                    }
                }
            }
        }
    } }
}


unsafe impl<T> WorldQuery for EqChanged<T>
where
    T : Component
{
    type Fetch<'a> = EqChangedFetch<'a, T>;
    type State     = EqChangedState<T>;

    fn shrink_fetch<'wlong : 'wshort, 'wshort>(
        fetch : Self::Fetch<'wlong>
    ) -> Self::Fetch<'wshort> {
        EqChangedFetch {
            world    : fetch.world,
            query_id : fetch.query_id,
            this_run : fetch.this_run,
            changed  : <Changed<T> as WorldQuery>::shrink_fetch(fetch.changed),
            old      : <Option<&mut PreviousValue<T>> as WorldQuery>::shrink_fetch(fetch.old),
            new      : <&T as WorldQuery>::shrink_fetch(fetch.new)
        }
    }

    unsafe fn init_fetch<'w>(
        world    : UnsafeWorldCell<'w>,
        state    : &Self::State,
        last_run : Tick,
        this_run : Tick,
    ) -> Self::Fetch<'w> { unsafe {
        EqChangedFetch {
            world    : world.into_deferred(),
            query_id : state.query_id,
            this_run,
            changed  : <Changed<T> as WorldQuery>::init_fetch(world, &state.changed, last_run, this_run),
            old      : <Option<&mut PreviousValue<T>> as WorldQuery>::init_fetch(world, &state.old, last_run, this_run),
            new      : <&T as WorldQuery>::init_fetch(world, &state.new, last_run, this_run)
        } }
    }

    const IS_DENSE : bool =
        <Changed<T> as WorldQuery>::IS_DENSE
        && <&mut PreviousValue<T> as WorldQuery>::IS_DENSE
        && <&T as WorldQuery>::IS_DENSE;

    unsafe fn set_archetype<'w>(
        fetch     : &mut Self::Fetch<'w>,
        state     : &Self::State,
        archetype : &'w Archetype,
        table     : &'w Table
    ) { unsafe {
        <Changed<T> as WorldQuery>::set_archetype(&mut fetch.changed, &state.changed, archetype, table);
        <Option<&mut PreviousValue<T>> as WorldQuery>::set_archetype(&mut fetch.old, &state.old, archetype, table);
        <&T as WorldQuery>::set_archetype(&mut fetch.new, &state.new, archetype, table);
    } }

    unsafe fn set_table<'w>(
        fetch : &mut Self::Fetch<'w>,
        state : &Self::State,
        table : &'w Table
    ) { unsafe {
        <Changed<T> as WorldQuery>::set_table(&mut fetch.changed, &state.changed, table);
        <Option<&mut PreviousValue<T>> as WorldQuery>::set_table(&mut fetch.old, &state.old, table);
        <&T as WorldQuery>::set_table(&mut fetch.new, &state.new, table);
    } }

    fn update_component_access(
        state  : &Self::State,
        access : &mut FilteredAccess<ComponentId>
    ) {
        <Changed<T> as WorldQuery>::update_component_access(&state.changed, access);
        <Option<&mut PreviousValue<T>> as WorldQuery>::update_component_access(&state.old, access);
        <&T as WorldQuery>::update_component_access(&state.new, access);
    }

    fn init_state(
        world : &mut World
    ) -> Self::State {
        EqChangedState {
            query_id : {
                let mut next_query_id = world.get_resource_or_insert_with(NextQueryId::default);
                let query_id = next_query_id.0;
                next_query_id.0 += 1;
                query_id
            },
            changed  : <Changed<T> as WorldQuery>::init_state(world),
            old      : <Option<&mut PreviousValue<T>> as WorldQuery>::init_state(world),
            new      : <&T as WorldQuery>::init_state(world)
        }
    }

    fn get_state(_components : &Components) -> Option<Self::State> {
        None
    }

    fn matches_component_set(
        state           : &Self::State,
        set_contains_id : &impl Fn(ComponentId) -> bool,
    ) -> bool {
        <Changed<T> as WorldQuery>::matches_component_set(&state.changed, set_contains_id)
        && <Option<&mut PreviousValue<T>> as WorldQuery>::matches_component_set(&state.new, set_contains_id)
        && <&T as WorldQuery>::matches_component_set(&state.new, set_contains_id)
    }
}
