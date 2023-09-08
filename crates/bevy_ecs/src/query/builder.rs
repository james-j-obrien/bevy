use bevy_utils::{all_indices, all_tuples};

use crate::prelude::World;

use super::*;

pub struct QueryBuilder<Q: WorldQuery, F: ReadOnlyWorldQuery = ()> {
    fetch_config: <Q as WorldQuery>::Config,
    filter_config: <F as WorldQuery>::Config,
}

impl<Q: WorldQuery, F: ReadOnlyWorldQuery> QueryBuilder<Q, F> {
    pub fn new() -> Self {
        Self {
            fetch_config: <<Q as WorldQuery>::Config as Default>::default(),
            filter_config: <<F as WorldQuery>::Config as Default>::default(),
        }
    }

    pub fn build(self, world: &mut World) -> QueryState<Q, F> {
        QueryState::new_with_config(world, self.fetch_config, self.filter_config)
    }

    pub fn config<const N: u32, T>(&mut self, value: T) -> &mut Self
    where
        Self: Config<N, T>,
    {
        <Self as Config<N, T>>::config(self, value)
    }

    pub fn with_config(&mut self, value: Q::Config) -> &mut Self {
        self.fetch_config = value;
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
    fn config(&mut self, value: T) -> &mut Self;
}

macro_rules! impl_config {
    ($config: ident, $index: literal, $field: tt, $($name: ident),*) => {
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        impl<$($name: WorldQuery,)*> Config<$index, $config::Config> for QueryBuilder<($($name,)*)> {
            fn config(&mut self, value: $config::Config) -> &mut Self {
                self.fetch_config.$field = value;
                self
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
    fn test_config() {
        let mut world = World::new();
        let entity = world.spawn((A(0), B(1))).id();
        let component_id_a = world.component_id::<A>().unwrap();
        let component_id_b = world.component_id::<B>().unwrap();

        let mut query = QueryBuilder::<(Entity, Ptr, Ptr)>::new();
        query.config::<1, _>(component_id_a);
        query.config::<2, _>(component_id_b);
        let mut state = query.build(&mut world);
        let (e, a, b) = state.single(&world);

        assert_eq!(e, entity);

        let a = unsafe { a.deref::<A>() };
        let b = unsafe { b.deref::<B>() };

        assert_eq!(a.0, 0);
        assert_eq!(b.0, 1);
    }
}
