//! Types for creating and storing [`Observer`]s

mod builder;
mod entity_observer;
mod runner;

use std::marker::PhantomData;

use crate::{
    archetype::ArchetypeFlags,
    query::DebugCheckedUnwrap,
    system::{EmitTrigger, IntoObserverSystem},
    world::*,
};
pub use bevy_ecs_macros::Trigger;
pub use builder::*;
pub(crate) use entity_observer::*;
pub use runner::*;

use bevy_ptr::{Ptr, PtrMut};
use bevy_utils::{EntityHashMap, HashMap};

use crate::{component::ComponentId, prelude::*, world::DeferredWorld};

/// Trait implemented for types that are used as ECS triggers observed by [`Observer`]
pub trait Trigger: Component {}

/// Type used in callbacks registered for observers.
pub struct Observer<'w, T, B: Bundle = ()> {
    data: &'w mut T,
    trigger: ObserverTrigger,
    _marker: PhantomData<B>,
}

impl<'w, T, B: Bundle> Observer<'w, T, B> {
    pub(crate) fn new(data: &'w mut T, trigger: ObserverTrigger) -> Self {
        Self {
            data,
            trigger,
            _marker: PhantomData,
        }
    }

    /// Returns the trigger id for the triggering trigger
    pub fn trigger(&self) -> ComponentId {
        self.trigger.trigger
    }

    /// Returns a reference to the data associated with the trigger that triggered the observer.
    pub fn data(&self) -> &T {
        self.data
    }

    /// Returns a mutable reference to the data associated with the trigger that triggered the observer.
    pub fn data_mut(&mut self) -> &mut T {
        self.data
    }

    /// Returns a pointer to the data associated with the trigger that triggered the observer.
    pub fn data_ptr(&self) -> Ptr {
        Ptr::from(&self.data)
    }

    /// Returns the entity that triggered the observer, could be [`Entity::PLACEHOLDER`].
    pub fn source(&self) -> Entity {
        self.trigger.source
    }
}

#[derive(Default, Clone)]
pub(crate) struct ObserverDescriptor {
    triggers: Vec<ComponentId>,
    components: Vec<ComponentId>,
    sources: Vec<Entity>,
}

/// Metadata for the source triggering an [`Observer`],
pub struct ObserverTrigger {
    observer: Entity,
    trigger: ComponentId,
    source: Entity,
}

// Map between an observer entity and it's runner
type ObserverMap = EntityHashMap<Entity, ObserverRunner>;

/// Collection of [`ObserverRunner`] for [`Observer`] registered to a particular trigger targeted at a specific component.
#[derive(Default, Debug)]
pub struct CachedComponentObservers {
    // Observers listening to triggers targeting this component
    map: ObserverMap,
    // Observers listening to triggers targeting this component on a specific entity
    entity_map: EntityHashMap<Entity, ObserverMap>,
}

/// Collection of [`ObserverRunner`] for [`Observer`] registered to a particular trigger.
#[derive(Default, Debug)]
pub struct CachedObservers {
    // Observers listening for any time this trigger is fired
    map: ObserverMap,
    // Observers listening for this trigger fired at a specific component
    component_observers: HashMap<ComponentId, CachedComponentObservers>,
    // Observers listening for this trigger fired at a specific entity
    entity_observers: EntityHashMap<Entity, ObserverMap>,
}

/// Metadata for observers. Stores a cache mapping trigger ids to the registered observers.
#[derive(Default, Debug)]
pub struct Observers {
    // Cached ECS observers to save a lookup most common triggers.
    on_add: CachedObservers,
    on_insert: CachedObservers,
    on_remove: CachedObservers,
    // Map from trigger type to set of observers
    cache: HashMap<ComponentId, CachedObservers>,
}

impl Observers {
    pub(crate) fn get_observers(&mut self, trigger: ComponentId) -> &mut CachedObservers {
        match trigger {
            ON_ADD => &mut self.on_add,
            ON_INSERT => &mut self.on_insert,
            ON_REMOVE => &mut self.on_remove,
            _ => self.cache.entry(trigger).or_default(),
        }
    }

    pub(crate) fn try_get_observers(&self, trigger: ComponentId) -> Option<&CachedObservers> {
        match trigger {
            ON_ADD => Some(&self.on_add),
            ON_INSERT => Some(&self.on_insert),
            ON_REMOVE => Some(&self.on_remove),
            _ => self.cache.get(&trigger),
        }
    }

    pub(crate) fn invoke<T>(
        mut world: DeferredWorld,
        trigger: ComponentId,
        source: Entity,
        components: impl Iterator<Item = ComponentId>,
        data: &mut T,
    ) {
        // SAFETY: You cannot get a mutable reference to `observers` from `DeferredWorld`
        let (mut world, observers) = unsafe {
            let world = world.as_unsafe_world_cell();
            // SAFETY: There are no outstanding world references
            world.increment_trigger_id();
            let observers = world.observers();
            let Some(observers) = observers.try_get_observers(trigger) else {
                return;
            };
            // SAFETY: The only outstanding reference to world is `observers`
            (world.into_deferred(), observers)
        };

        let mut trigger_observer = |(&observer, runner): (&Entity, &ObserverRunner)| {
            (runner)(
                world.reborrow(),
                ObserverTrigger {
                    observer,
                    trigger,
                    source,
                },
                data.into(),
            );
        };

        // Trigger observers listening for any kind of this trigger
        observers.map.iter().for_each(&mut trigger_observer);

        // Trigger entity observers listening for this kind of trigger
        if source != Entity::PLACEHOLDER {
            if let Some(map) = observers.entity_observers.get(&source) {
                map.iter().for_each(&mut trigger_observer);
            }
        }

        // Trigger observers listening to this trigger targeting a specific component
        components.for_each(|id| {
            if let Some(component_observers) = observers.component_observers.get(&id) {
                component_observers
                    .map
                    .iter()
                    .for_each(&mut trigger_observer);

                if source != Entity::PLACEHOLDER {
                    if let Some(map) = component_observers.entity_map.get(&source) {
                        map.iter().for_each(&mut trigger_observer);
                    }
                }
            }
        });
    }

    pub(crate) fn is_archetype_cached(trigger: ComponentId) -> Option<ArchetypeFlags> {
        match trigger {
            ON_ADD => Some(ArchetypeFlags::ON_ADD_OBSERVER),
            ON_INSERT => Some(ArchetypeFlags::ON_INSERT_OBSERVER),
            ON_REMOVE => Some(ArchetypeFlags::ON_REMOVE_OBSERVER),
            _ => None,
        }
    }

    pub(crate) fn update_archetype_flags(
        &self,
        component_id: ComponentId,
        flags: &mut ArchetypeFlags,
    ) {
        if self.on_add.component_observers.contains_key(&component_id) {
            flags.insert(ArchetypeFlags::ON_ADD_OBSERVER);
        }
        if self
            .on_insert
            .component_observers
            .contains_key(&component_id)
        {
            flags.insert(ArchetypeFlags::ON_INSERT_OBSERVER);
        }
        if self
            .on_remove
            .component_observers
            .contains_key(&component_id)
        {
            flags.insert(ArchetypeFlags::ON_REMOVE_OBSERVER);
        }
    }
}

impl World {
    /// Construct an [`ObserverBuilder`]
    pub fn observer_builder<T: Trigger>(&mut self) -> ObserverBuilder<T> {
        self.init_trigger::<T>();
        ObserverBuilder::new(self.commands())
    }

    /// Spawn an [`Observer`] and returns it's [`Entity`].
    pub fn observer<T: Component, B: Bundle, M>(
        &mut self,
        system: impl IntoObserverSystem<T, B, M>,
    ) -> Entity {
        // Ensure triggers are registered with the world
        // Note: this sidesteps init_trigger
        B::component_ids(&mut self.components, &mut self.storages, &mut |_| {});
        ObserverBuilder::new(self.commands()).run(system)
    }

    /// Registers `T` as a [`Trigger`]
    pub fn init_trigger<T: Trigger>(&mut self) {
        self.init_component::<T>();
    }

    /// Constructs a [`TriggerBuilder`].
    pub fn trigger<T: Trigger>(&mut self, trigger: T) -> TriggerBuilder<T> {
        self.init_component::<T>();
        TriggerBuilder::new(trigger, self.commands())
    }

    /// Register an observer to the cache, called when an observer is created
    pub(crate) fn register_observer(&mut self, entity: Entity) {
        // SAFETY: References do not alias.
        let (observer_component, archetypes, observers) = unsafe {
            let observer_component: *const ObserverComponent =
                self.get::<ObserverComponent>(entity).unwrap();
            (
                &*observer_component,
                &mut self.archetypes,
                &mut self.observers,
            )
        };
        let descriptor = &observer_component.descriptor;

        for &trigger in &descriptor.triggers {
            let cache = observers.get_observers(trigger);

            if descriptor.components.is_empty() && descriptor.sources.is_empty() {
                cache.map.insert(entity, observer_component.runner);
            } else if descriptor.components.is_empty() {
                // Observer is not targeting any components so register it as an entity observer
                for &source in &observer_component.descriptor.sources {
                    let map = cache.entity_observers.entry(source).or_default();
                    map.insert(entity, observer_component.runner);
                }
            } else {
                // Register observer for each source component
                for &component in &descriptor.components {
                    let observers =
                        cache
                            .component_observers
                            .entry(component)
                            .or_insert_with(|| {
                                if let Some(flag) = Observers::is_archetype_cached(trigger) {
                                    archetypes.update_flags(component, flag, true);
                                }
                                CachedComponentObservers::default()
                            });
                    if descriptor.sources.is_empty() {
                        // Register for all triggers targeting the component
                        observers.map.insert(entity, observer_component.runner);
                    } else {
                        // Register for each targeted entity
                        for &source in &descriptor.sources {
                            let map = observers.entity_map.entry(source).or_default();
                            map.insert(entity, observer_component.runner);
                        }
                    }
                }
            }
        }
    }

    /// Remove the observer from the cache, called when an observer gets despawned
    pub(crate) fn unregister_observer(&mut self, entity: Entity, descriptor: ObserverDescriptor) {
        let archetypes = &mut self.archetypes;
        let observers = &mut self.observers;

        for &trigger in &descriptor.triggers {
            let cache = observers.get_observers(trigger);
            if descriptor.components.is_empty() && descriptor.sources.is_empty() {
                cache.map.remove(&entity);
            } else if descriptor.components.is_empty() {
                for source in &descriptor.sources {
                    // This check should be unnecessary since this observer hasn't been unregistered yet
                    let Some(observers) = cache.entity_observers.get_mut(source) else {
                        continue;
                    };
                    observers.remove(&entity);
                    if observers.is_empty() {
                        cache.entity_observers.remove(source);
                    }
                }
            } else {
                for component in &descriptor.components {
                    let Some(observers) = cache.component_observers.get_mut(component) else {
                        continue;
                    };
                    if descriptor.sources.is_empty() {
                        observers.map.remove(&entity);
                    } else {
                        for source in &descriptor.sources {
                            let Some(map) = observers.entity_map.get_mut(source) else {
                                continue;
                            };
                            map.remove(&entity);
                            if map.is_empty() {
                                observers.entity_map.remove(source);
                            }
                        }
                    }

                    if observers.map.is_empty() && observers.entity_map.is_empty() {
                        cache.component_observers.remove(component);
                        if let Some(flag) = Observers::is_archetype_cached(trigger) {
                            archetypes.update_flags(*component, flag, false);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_ptr::OwningPtr;

    use crate as bevy_ecs;
    use crate::component::ComponentDescriptor;
    use crate::observer::TriggerBuilder;
    use crate::prelude::*;

    use super::ObserverBuilder;

    #[derive(Component)]
    struct A;

    #[derive(Component)]
    struct B;

    #[derive(Component)]
    struct C;

    #[derive(Trigger)]
    struct TriggerA;

    #[derive(Resource, Default)]
    struct R(usize);

    impl R {
        #[track_caller]
        fn assert_order(&mut self, count: usize) {
            assert_eq!(count, self.0);
            self.0 += 1;
        }
    }

    #[test]
    fn observer_order_spawn_despawn() {
        let mut world = World::new();
        world.init_resource::<R>();

        world.observer(|_: Observer<OnAdd, A>, mut res: ResMut<R>| res.assert_order(0));
        world.observer(|_: Observer<OnInsert, A>, mut res: ResMut<R>| res.assert_order(1));
        world.observer(|_: Observer<OnRemove, A>, mut res: ResMut<R>| res.assert_order(2));

        let entity = world.spawn(A).id();
        world.despawn(entity);
        assert_eq!(3, world.resource::<R>().0);
    }

    #[test]
    fn observer_order_insert_remove() {
        let mut world = World::new();
        world.init_resource::<R>();

        world.observer(|_: Observer<OnAdd, A>, mut res: ResMut<R>| res.assert_order(0));
        world.observer(|_: Observer<OnInsert, A>, mut res: ResMut<R>| res.assert_order(1));
        world.observer(|_: Observer<OnRemove, A>, mut res: ResMut<R>| res.assert_order(2));

        let mut entity = world.spawn_empty();
        entity.insert(A);
        entity.remove::<A>();
        entity.flush();
        assert_eq!(3, world.resource::<R>().0);
    }

    #[test]
    fn observer_order_recursive() {
        let mut world = World::new();
        world.init_resource::<R>();
        world.observer(
            |obs: Observer<OnAdd, A>, mut res: ResMut<R>, mut commands: Commands| {
                res.assert_order(0);
                commands.entity(obs.source()).insert(B);
            },
        );
        world.observer(
            |obs: Observer<OnRemove, A>, mut res: ResMut<R>, mut commands: Commands| {
                res.assert_order(2);
                commands.entity(obs.source()).remove::<B>();
            },
        );

        world.observer(
            |obs: Observer<OnAdd, B>, mut res: ResMut<R>, mut commands: Commands| {
                res.assert_order(1);
                commands.entity(obs.source()).remove::<A>();
            },
        );
        world.observer(|_: Observer<OnRemove, B>, mut res: ResMut<R>| {
            res.assert_order(3);
        });

        let entity = world.spawn(A).flush();
        let entity = world.get_entity(entity).unwrap();
        assert!(!entity.contains::<A>());
        assert!(!entity.contains::<B>());
        assert_eq!(4, world.resource::<R>().0);
    }

    #[test]
    fn observer_multiple_listeners() {
        let mut world = World::new();
        world.init_resource::<R>();

        world.observer(|_: Observer<OnAdd, A>, mut res: ResMut<R>| res.0 += 1);
        world.observer(|_: Observer<OnAdd, A>, mut res: ResMut<R>| res.0 += 1);

        world.spawn(A).flush();
        assert_eq!(2, world.resource::<R>().0);
        // Our A entity plus our two observers
        assert_eq!(world.entities().len(), 3);
    }

    #[test]
    fn observer_multiple_triggerss() {
        let mut world = World::new();
        world.init_resource::<R>();
        world.init_component::<A>();

        world
            .observer_builder::<OnAdd>()
            .on_trigger::<OnRemove>()
            .run(|_: Observer<_, A>, mut res: ResMut<R>| res.0 += 1);

        let entity = world.spawn(A).id();
        world.despawn(entity);
        assert_eq!(2, world.resource::<R>().0);
    }

    #[test]
    fn observer_multiple_components() {
        let mut world = World::new();
        world.init_resource::<R>();
        world.init_component::<A>();
        world.init_component::<B>();

        world.observer(|_: Observer<OnAdd, (A, B)>, mut res: ResMut<R>| res.0 += 1);

        let entity = world.spawn(A).id();
        world.entity_mut(entity).insert(B);
        world.flush();
        assert_eq!(2, world.resource::<R>().0);
    }

    #[test]
    fn observer_despawn() {
        let mut world = World::new();
        world.init_resource::<R>();

        let observer = world
            .observer(|_: Observer<OnAdd, A>| panic!("Observer triggered after being despawned."));
        world.despawn(observer);
        world.spawn(A).flush();
    }

    #[test]
    fn observer_multiple_matches() {
        let mut world = World::new();
        world.init_resource::<R>();

        world.observer(|_: Observer<OnAdd, (A, B)>, mut res: ResMut<R>| res.0 += 1);

        world.spawn((A, B)).flush();
        assert_eq!(1, world.resource::<R>().0);
    }

    #[test]
    fn observer_no_source() {
        let mut world = World::new();
        world.init_resource::<R>();
        world.init_trigger::<TriggerA>();

        world
            .spawn_empty()
            .observe(|_: Observer<TriggerA>| panic!("Trigger routed to non-targeted entity."));
        world.observer(move |obs: Observer<TriggerA>, mut res: ResMut<R>| {
            assert_eq!(obs.source(), Entity::PLACEHOLDER);
            res.0 += 1;
        });

        world.trigger(TriggerA).emit();
        world.flush();
        assert_eq!(1, world.resource::<R>().0);
    }

    #[test]
    fn observer_entity_routing() {
        let mut world = World::new();
        world.init_resource::<R>();
        world.init_trigger::<TriggerA>();

        world
            .spawn_empty()
            .observe(|_: Observer<TriggerA>| panic!("Trigger routed to non-targeted entity."));
        let entity = world
            .spawn_empty()
            .observe(|_: Observer<TriggerA>, mut res: ResMut<R>| res.0 += 1)
            .id();
        world.observer(move |obs: Observer<TriggerA>, mut res: ResMut<R>| {
            assert_eq!(obs.source(), entity);
            res.0 += 1;
        });

        world.trigger(TriggerA).entity(entity).emit();
        world.flush();
        assert_eq!(2, world.resource::<R>().0);
    }

    #[test]
    fn observer_dynamic_component() {
        let mut world = World::new();
        world.init_resource::<R>();

        let component_id = world.init_component_with_descriptor(ComponentDescriptor::new::<A>());
        world
            .observer_builder()
            .component_ids(&[component_id])
            .run(|_: Observer<OnAdd>, mut res: ResMut<R>| res.0 += 1);

        let mut entity = world.spawn_empty();
        OwningPtr::make(A, |ptr| {
            // SAFETY: we registered `component_id` above.
            unsafe { entity.insert_by_id(component_id, ptr) };
        });
        let entity = entity.flush();

        world.trigger(TriggerA).entity(entity).emit();
        world.flush();
        assert_eq!(1, world.resource::<R>().0);
    }

    #[test]
    fn observer_dynamic_trigger() {
        let mut world = World::new();
        world.init_resource::<R>();

        let trigger = world.init_component_with_descriptor(ComponentDescriptor::new::<TriggerA>());
        // SAFETY: we registered `trigger` above
        unsafe { ObserverBuilder::new_with_id(trigger, world.commands()) }
            .run(|_: Observer<TriggerA>, mut res: ResMut<R>| res.0 += 1);

        // SAFETY: we registered `trigger` above
        unsafe { TriggerBuilder::new_with_id(trigger, TriggerA, world.commands()) }.emit();
        world.flush();
        assert_eq!(1, world.resource::<R>().0);
    }
}
