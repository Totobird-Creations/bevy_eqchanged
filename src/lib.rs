use core::marker::PhantomData;
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
use intmap::{
    IntMap,
    Entry as IntMapEntry
};


#[derive(Resource, Default)]
struct NextQueryId(usize);

#[derive(Component)]
struct PreviousValues<T> {
    by_query_id : IntMap<usize, T>
}
impl<T> Default for PreviousValues<T> {
    fn default() -> Self {
        Self { by_query_id : IntMap::new() }
    }
}


pub struct EqChanged<T>(PhantomData<T>);


pub struct EqChangedState<T>
where
    T : Component
{
    query_id : usize,
    changed  : <Changed<T> as WorldQuery>::State,
    old      : <Option<&'static mut PreviousValues<T>> as WorldQuery>::State,
    new      : <&'static T as WorldQuery>::State
}

pub struct EqChangedFetch<'l, T>
where
    T : Component
{
    world    : DeferredWorld<'l>,
    query_id : usize,
    changed  : <Changed<T> as WorldQuery>::Fetch<'l>,
    old      : <Option<&'static mut PreviousValues<T>> as WorldQuery>::Fetch<'l>,
    new      : <&'static T as WorldQuery>::Fetch<'l>
}

impl<'l, T> Clone for EqChangedFetch<'l, T>
where
    T : Component
{
    fn clone(&self) -> Self { unreachable!() }
}


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
        let old = <Option<&mut PreviousValues<T>> as QueryData>::fetch(&mut fetch.old, entity, table_row);
        let new = <&T as QueryData>::fetch(&mut fetch.new, entity, table_row);
        match (old) {
            // PreviousValues<T> was not in the current query.
            None => {
                let qid = fetch.query_id;
                let new = new.clone();
                // Spawn a new PreviousValues<T>, or edit it if it already exists.
                fetch.world.commands().entity(entity).queue(move |mut world : EntityWorldMut| {
                    match (world.get_mut::<PreviousValues<T>>()) {
                        Some(mut previous_values) => {
                            previous_values.by_query_id.insert(qid, new);
                        },
                        None => {
                            let mut by_query_id = IntMap::new();
                            by_query_id.insert(qid, new);
                            world.insert(PreviousValues { by_query_id });
                        }
                    }
                });
                true
            },
            // PreviousValues<T> was in the current query.
            Some(mut old) => {
                match (old.by_query_id.entry(fetch.query_id)) {
                    IntMapEntry::Occupied(mut old_entry) => {
                        if (old_entry.get() != new) {
                            old_entry.insert(new.clone());
                            true
                        } else { false }
                    },
                    IntMapEntry::Vacant(old_entry) => {
                        old_entry.insert(new.clone());
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
            changed  : <Changed<T> as WorldQuery>::shrink_fetch(fetch.changed),
            old      : <Option<&mut PreviousValues<T>> as WorldQuery>::shrink_fetch(fetch.old),
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
            changed  : <Changed<T> as WorldQuery>::init_fetch(world, &state.changed, last_run, this_run),
            old      : <Option<&mut PreviousValues<T>> as WorldQuery>::init_fetch(world, &state.old, last_run, this_run),
            new      : <&T as WorldQuery>::init_fetch(world, &state.new, last_run, this_run)
        } }
    }

    const IS_DENSE : bool =
        <Changed<T> as WorldQuery>::IS_DENSE
        && <&mut PreviousValues<T> as WorldQuery>::IS_DENSE
        && <&T as WorldQuery>::IS_DENSE;

    unsafe fn set_archetype<'w>(
        fetch     : &mut Self::Fetch<'w>,
        state     : &Self::State,
        archetype : &'w Archetype,
        table     : &'w Table
    ) { unsafe {
        <Changed<T> as WorldQuery>::set_archetype(&mut fetch.changed, &state.changed, archetype, table);
        <Option<&mut PreviousValues<T>> as WorldQuery>::set_archetype(&mut fetch.old, &state.old, archetype, table);
        <&T as WorldQuery>::set_archetype(&mut fetch.new, &state.new, archetype, table);
    } }

    unsafe fn set_table<'w>(
        fetch : &mut Self::Fetch<'w>,
        state : &Self::State,
        table : &'w Table
    ) { unsafe {
        <Changed<T> as WorldQuery>::set_table(&mut fetch.changed, &state.changed, table);
        <Option<&mut PreviousValues<T>> as WorldQuery>::set_table(&mut fetch.old, &state.old, table);
        <&T as WorldQuery>::set_table(&mut fetch.new, &state.new, table);
    } }

    fn update_component_access(
        state  : &Self::State,
        access : &mut FilteredAccess<ComponentId>
    ) {
        <Changed<T> as WorldQuery>::update_component_access(&state.changed, access);
        <Option<&mut PreviousValues<T>> as WorldQuery>::update_component_access(&state.old, access);
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
            old      : <Option<&mut PreviousValues<T>> as WorldQuery>::init_state(world),
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
        && <Option<&mut PreviousValues<T>> as WorldQuery>::matches_component_set(&state.new, set_contains_id)
        && <&T as WorldQuery>::matches_component_set(&state.new, set_contains_id)
    }
}
