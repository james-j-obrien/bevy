use crate::{
    archetype::{Archetype, ArchetypeComponentId},
    component::{ComponentId, Tick},
    entity::Entity,
    prelude::{AnyOf, Or, World},
    query::{Access, DebugCheckedUnwrap, FilteredAccess},
    storage::{Table, TableRow},
    term_query::TermVec,
    world::unsafe_world_cell::UnsafeWorldCell,
};

use super::{
    ComponentQueryTermGroup, ComponentTerm, ComponentTermState, Fetchable, FetchedComponent,
    FetchedTerm, QueryTermGroup, Term,
};

#[derive(Clone)]
pub struct OrTerm {
    terms: Vec<ComponentTerm>,
    fetch: bool,
}

pub struct OrTermState<'w> {
    component: ComponentTermState<'w>,
    matches: bool,
}

impl OrTermState<'_> {
    #[inline]
    pub fn dense(&self) -> bool {
        !self.matches || self.component.dense()
    }
}

impl Fetchable for OrTerm {
    type State<'w> = Vec<OrTermState<'w>>;
    type Item<'w> = Vec<FetchedComponent<'w>>;

    #[inline]
    unsafe fn init_state<'w>(
        &self,
        world: UnsafeWorldCell<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Vec<OrTermState<'w>> {
        self.terms
            .iter()
            .map(|term| OrTermState {
                component: term.init_state(world, last_run, this_run),
                matches: false,
            })
            .collect()
    }

    #[inline]
    unsafe fn set_table<'w>(&self, state: &mut Self::State<'w>, table: &'w Table) {
        self.terms
            .iter()
            .zip(state.iter_mut())
            .for_each(|(term, state)| {
                state.matches = term.matches_component_set(&|id| table.has_column(id));
                if state.matches {
                    term.set_table(&mut state.component, table)
                }
            })
    }

    #[inline(always)]
    unsafe fn fetch<'w>(
        &self,
        state: &Self::State<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w> {
        if self.fetch {
            self.terms
                .iter()
                .zip(state.iter())
                .map(|(term, state)| term.fetch(&state.component, entity, table_row))
                .collect()
        } else {
            Vec::new()
        }
    }

    #[inline(always)]
    unsafe fn filter_fetch<'w>(
        &self,
        state: &Self::State<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        self.terms.iter().zip(state.iter()).any(|(term, state)| {
            state.matches && term.filter_fetch(&state.component, entity, table_row)
        })
    }

    #[inline]
    fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
        let mut iter = self.terms.iter();
        let Some(term) = iter.next() else {
            return
        };
        let mut new_access = access.clone();
        term.update_component_access(&mut new_access);
        self.terms.iter().for_each(|term| {
            let mut intermediate = access.clone();
            term.update_component_access(&mut intermediate);
            new_access.append_or(&intermediate);
            new_access.extend_access(&intermediate);
        });
        *access = new_access;
    }

    #[inline]
    fn update_archetype_component_access(
        &self,
        archetype: &Archetype,
        access: &mut Access<ArchetypeComponentId>,
    ) {
        self.terms
            .iter()
            .for_each(|term| term.update_archetype_component_access(archetype, access))
    }

    #[inline]
    fn matches_component_set(&self, set_contains_id: &impl Fn(ComponentId) -> bool) -> bool {
        self.terms
            .iter()
            .any(|term| term.matches_component_set(set_contains_id))
    }
}

impl<Q: ComponentQueryTermGroup> QueryTermGroup for Or<Q> {
    type Item<'w> = ();
    type ReadOnly = Self;

    fn init_terms(world: &mut World, terms: &mut TermVec<Term>) {
        let mut sub_terms = Vec::new();
        Q::init_terms(world, &mut sub_terms);
        terms.push(Term::Or(OrTerm {
            terms: sub_terms,
            fetch: false,
        }));
    }

    #[inline(always)]
    unsafe fn from_fetches<'w>(
        terms: &mut impl Iterator<Item = FetchedTerm<'w>>,
    ) -> Self::Item<'w> {
        terms.next();
    }
}

impl<Q: ComponentQueryTermGroup> QueryTermGroup for AnyOf<Q> {
    type Item<'w> = <Q::Optional as ComponentQueryTermGroup>::Item<'w>;
    type ReadOnly = Self;

    fn init_terms(world: &mut World, terms: &mut TermVec<Term>) {
        let mut sub_terms = Vec::new();
        Q::Optional::init_terms(world, &mut sub_terms);
        terms.push(Term::Or(OrTerm {
            terms: sub_terms,
            fetch: true,
        }));
    }

    #[inline(always)]
    unsafe fn from_fetches<'w>(
        terms: &mut impl Iterator<Item = FetchedTerm<'w>>,
    ) -> Self::Item<'w> {
        let term = terms
            .next()
            .debug_checked_unwrap()
            .group()
            .debug_checked_unwrap();
        Q::Optional::from_fetches(&mut term.into_iter())
    }
}
