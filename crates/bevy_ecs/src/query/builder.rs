use std::ops::{Deref, DerefMut};

use bevy_utils::{all_indices, all_tuples};

use crate::{
    component::ComponentId,
    prelude::{Component, World},
};

#[derive(Default, Clone)]
pub enum TermAccess {
    #[default]
    None,
    Read,
    Write,
}

#[derive(Default, Clone)]
pub enum TermOperator {
    #[default]
    With,
    Without,
    Any,
}

#[derive(Default, Clone)]
pub struct Term {
    id: Option<ComponentId>,
    pub access: TermAccess,
    pub oper: TermOperator,
}

impl Term {
    pub fn none_id(id: ComponentId) -> Self {
        Self {
            id: Some(id),
            access: TermAccess::None,
            oper: TermOperator::Any,
        }
    }
    pub fn read_id(id: ComponentId) -> Self {
        Self {
            id: Some(id),
            access: TermAccess::Read,
            oper: TermOperator::With,
        }
    }

    pub fn read() -> Self {
        Self {
            id: None,
            access: TermAccess::Read,
            oper: TermOperator::With,
        }
    }

    pub fn write_id(id: ComponentId) -> Self {
        Self {
            id: Some(id),
            access: TermAccess::Write,
            oper: TermOperator::With,
        }
    }

    pub fn write() -> Self {
        Self {
            id: None,
            access: TermAccess::Write,
            oper: TermOperator::With,
        }
    }

    pub fn with_id(id: ComponentId) -> Self {
        Self {
            id: Some(id),
            access: TermAccess::None,
            oper: TermOperator::With,
        }
    }

    pub fn without_id(id: ComponentId) -> Self {
        Self {
            id: Some(id),
            access: TermAccess::None,
            oper: TermOperator::Without,
        }
    }

    pub fn set_id(&mut self, id: ComponentId) {
        self.id = Some(id);
    }

    pub fn id(&self) -> ComponentId {
        self.id.unwrap()
    }

    pub fn matches_component_set(
        terms: &Vec<Term>,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        terms.iter().all(|term| match term.oper {
            TermOperator::With => set_contains_id(term.id()),
            TermOperator::Without => !set_contains_id(term.id()),
            TermOperator::Any => true,
        })
    }
}

use super::*;

pub struct TermBuilder<'w> {
    term: &'w mut Term,
    world: &'w mut World,
}

impl<'w> Deref for TermBuilder<'w> {
    type Target = &'w mut Term;

    fn deref(&self) -> &Self::Target {
        &self.term
    }
}

impl<'w> DerefMut for TermBuilder<'w> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.term
    }
}

impl<'w> TermBuilder<'w> {
    pub fn new(term: &'w mut Term, world: &'w mut World) -> Self {
        Self { term, world }
    }

    pub fn set<T: Component>(&mut self) {
        self.term.set_id(self.world.init_component::<T>());
    }
}

pub struct QueryBuilder<'w, Q: WorldQuery> {
    state: <Q as WorldQuery>::State,
    terms: Vec<Term>,
    world: &'w mut World,
}

impl<'w, Q: WorldQuery> QueryBuilder<'w, Q> {
    pub fn new(world: &'w mut World) -> Self {
        Self {
            state: Q::init_state(world),
            terms: Vec::new(),
            world,
        }
    }

    pub fn build(&mut self) -> QueryState<Q> {
        let state = self.state.clone();
        let terms = self.terms.clone();
        QueryState::new_with_state(self.world, state, (), terms)
    }

    pub fn config<const N: u32, T>(&mut self, f: impl Fn(&mut T)) -> &mut Self
    where
        Self: Config<N, T>,
    {
        <Self as Config<N, T>>::config(self, f)
    }

    pub fn term<const N: u32>(&mut self, f: impl Fn(&mut TermBuilder)) -> &mut Self
    where
        Self: Config<N, Term>,
    {
        let (term, world) = <Self as Config<N, Term>>::config_mut(self);
        f(&mut TermBuilder::new(term, world));
        self
    }

    pub fn with<T: Component>(&mut self) -> &mut Self {
        let id = self.world.init_component::<T>();
        self.with_id(id);
        self
    }

    pub fn with_id(&mut self, id: ComponentId) -> &mut Self {
        self.terms.push(Term::with_id(id));
        self
    }

    pub fn without<T: Component>(&mut self) -> &mut Self {
        let id = self.world.init_component::<T>();
        self.without_id(id);
        self
    }

    pub fn without_id(&mut self, id: ComponentId) -> &mut Self {
        self.terms.push(Term::without_id(id));
        self
    }
}

// impl<A: WorldQuery, B: WorldQuery> Config<0, A::Config> for QueryBuilder<(A, B)> {
//     fn config(&mut self, value: A::Config) -> &mut Self {
//         self.fetch_config.0 = value;
//         self
//     }
// }

// impl<A: WorldQuery, B: WorldQuery> Config<1, B::Config> for QueryBuilder<(A, B)> {
//     fn config(&mut self, value: B::Config) -> &mut Self {
//         self.fetch_config.1 = value;
//         self
//     }
// }

pub trait Config<const N: u32, T> {
    fn config(&mut self, f: impl Fn(&mut T)) -> &mut Self;

    fn config_mut(&mut self) -> (&mut T, &mut World);
}

macro_rules! impl_config {
    ($config: ident, $index: literal, $field: tt, $($name: ident),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<'w, $($name: WorldQuery,)*> Config<$index, $config::State> for QueryBuilder<'w, ($($name,)*)> {
            fn config(&mut self, f: impl Fn(&mut $config::State)) -> &mut Self {
                f(&mut self.state.$field);
                self
            }

            fn config_mut(&mut self) -> (&mut $config::State, &mut World)  {
                (&mut self.state.$field, self.world)
            }
        }
    };
}

macro_rules! impl_config_tuple {
    ($($name: ident),*) => {
        all_indices!(impl_config, $($name),*);
    };
}

all_tuples!(impl_config_tuple, 2, 12, F);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{self as bevy_ecs, prelude::*};
    use bevy_ptr::Ptr;

    #[derive(Component)]
    struct A(usize);

    #[derive(Component)]
    struct B(usize);

    #[test]
    fn test_builder_static() {
        let mut world = World::new();
        let entity = world.spawn((A(0), B(1))).id();

        let mut query = QueryBuilder::<(Entity, Ptr, Ptr)>::new(&mut world)
            .term::<1>(|t| t.set::<A>())
            .term::<2>(|t| t.set::<B>())
            .build();

        let (e, a, b) = query.single(&world);

        assert_eq!(e, entity);

        let a = unsafe { a.deref::<A>() };
        let b = unsafe { b.deref::<B>() };

        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
    }

    #[test]
    fn test_builder_dynamic() {
        let mut world = World::new();
        let entity = world.spawn((A(0), B(1))).id();
        let component_id_a = world.component_id::<A>().unwrap();
        let component_id_b = world.component_id::<B>().unwrap();

        let mut query = QueryBuilder::<(Entity, Ptr, Ptr)>::new(&mut world)
            .term::<1>(|t| t.set_id(component_id_a))
            .term::<2>(|t| t.set_id(component_id_b))
            .build();

        let (e, a, b) = query.single(&world);

        assert_eq!(e, entity);

        let a = unsafe { a.deref::<A>() };
        let b = unsafe { b.deref::<B>() };

        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
    }

    #[test]
    fn test_builder_with_without() {
        let mut world = World::new();
        let entity_a = world.spawn(A(0)).id();
        let entity_b = world.spawn(B(0)).id();

        let mut query_a = QueryBuilder::<Entity>::new(&mut world).with::<A>().build();
        assert_eq!(entity_a, query_a.single(&world));

        let mut query_b = QueryBuilder::<Entity>::new(&mut world)
            .without::<A>()
            .build();
        assert_eq!(entity_b, query_b.single(&world));
    }
}
