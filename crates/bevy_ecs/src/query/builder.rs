use crate::{
    component::ComponentId,
    prelude::{Component, World},
};
use bevy_derive::{Deref, DerefMut};
use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use super::*;

pub struct ComponentTerm {
    id: Option<ComponentId>,
}

impl ComponentTerm {
    pub fn new(id: ComponentId) -> Self {
        Self { id: Some(id) }
    }

    pub fn empty() -> Self {
        Self { id: None }
    }
}

#[derive(Default)]
pub enum Term {
    #[default]
    Entity,
    Group(Vec<Term>),
    Component(ComponentTerm),
}

impl Term {
    pub fn new(component: ComponentId) -> Self {
        Self::Component(ComponentTerm::new(component))
    }

    pub fn component() -> Self {
        Self::Component(ComponentTerm::empty())
    }

    pub fn group(terms: Vec<Term>) -> Self {
        Self::Group(terms)
    }

    pub fn id(&self) -> ComponentId {
        match &self {
            Term::Component(term) => term.id.unwrap(),
            _ => unreachable!(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Term> {
        match &self {
            Term::Group(terms) => terms.iter(),
            _ => unreachable!(),
        }
    }
}

impl Index<usize> for Term {
    type Output = Term;

    fn index(&self, index: usize) -> &Self::Output {
        match &self {
            Term::Group(terms) => &terms[index],
            _ => unreachable!(),
        }
    }
}

impl IndexMut<usize> for Term {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match self {
            Term::Group(terms) => &mut terms[index],
            _ => unreachable!(),
        }
    }
}

impl<'w> IntoIterator for Term {
    type Item = Term;
    type IntoIter = <Vec<Term> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Term::Group(terms) => terms.into_iter(),
            _ => unreachable!(),
        }
    }
}

pub struct QueryBuilder<'w, Q: WorldQuery> {
    world: &'w mut World,
    term: Term,
    _marker: PhantomData<Q>,
}

#[derive(Deref, DerefMut)]
pub struct TermBuilder<'w, Q: WorldQuery> {
    term: Term,
    path: Vec<usize>,
    #[deref]
    query: &'w mut QueryBuilder<'w, Q>,
}

impl<'w, Q: WorldQuery> Drop for TermBuilder<'w, Q> {
    fn drop(&mut self) {
        *self.query.term_path(&self.path) = std::mem::take(&mut self.term);
    }
}

impl<'w, Q: WorldQuery> TermBuilder<'w, Q> {
    pub fn new(term: Term, path: Vec<usize>, query: &'w mut QueryBuilder<'w, Q>) -> Self {
        Self { term, path, query }
    }

    pub fn id(&mut self, id: ComponentId) -> &mut Self {
        match &mut self.term {
            Term::Component(term) => term.id = Some(id),
            _ => panic!("Cannot set id on non-component term."),
        }
        self
    }

    pub fn set<T: Component>(&mut self) -> &mut Self {
        let id = self.query.world.init_component::<T>();
        self.id(id)
    }
}

impl<'w, Q: WorldQuery> QueryBuilder<'w, Q> {
    pub fn new(world: &'w mut World) -> Self {
        let term = Q::init_state(world);
        Self {
            world,
            term,
            _marker: PhantomData::default(),
        }
    }

    pub fn term(&'w mut self, index: usize) -> TermBuilder<'w, Q> {
        let term = std::mem::replace(&mut self.term[index], Term::Entity);
        TermBuilder::new(term, vec![index], self)
    }

    pub fn term_path(&mut self, path: &Vec<usize>) -> &mut Term {
        let mut term = &mut self.term;
        for &index in path {
            term = &mut term[index];
        }
        term
    }

    pub fn build(&mut self) -> QueryState<Q> {
        let term = std::mem::replace(&mut self.term, Term::Entity);
        QueryState::from(self.world, term)
    }
}

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
    fn test_static() {
        let mut world = World::new();
        let entity = world.spawn((A(0), B(1))).id();

        let mut query = QueryBuilder::<(Entity, Ptr, Ptr)>::new(&mut world)
            .term(1)
            .set::<A>()
            .term(2)
            .set::<B>()
            .build();

        let (e, a, b) = query.single(&world);

        assert_eq!(e, entity);

        let a = unsafe { a.deref::<A>() };
        let b = unsafe { b.deref::<B>() };

        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
    }

    #[test]
    fn test_dynamic() {
        let mut world = World::new();
        let entity = world.spawn((A(0), B(1))).id();
        let component_id_a = world.component_id::<A>().unwrap();
        let component_id_b = world.component_id::<B>().unwrap();

        let mut query = QueryBuilder::<(Entity, Ptr, Ptr)>::new(&mut world)
            .term(1)
            .id(component_id_a)
            .term(2)
            .id(component_id_b)
            .build();

        let (e, a, b) = query.single(&world);

        assert_eq!(e, entity);

        let a = unsafe { a.deref::<A>() };
        let b = unsafe { b.deref::<B>() };

        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
    }
}
