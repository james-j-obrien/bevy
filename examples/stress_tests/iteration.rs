use bevy::prelude::*;
use criterion::black_box;

#[derive(Component, Copy, Clone)]
struct Transform(Mat4);

#[derive(Component, Copy, Clone)]
struct Position(Vec3);

#[derive(Component, Copy, Clone)]
struct Rotation(Vec3);

#[derive(Component, Copy, Clone)]
struct Velocity(Vec3);

fn main() {
    let mut world = World::new();

    world.spawn_batch((0..10_000).map(|_| {
        black_box((
            Transform(Mat4::from_scale(Vec3::ONE)),
            Position(Vec3::X),
            Rotation(Vec3::X),
            Velocity(Vec3::X),
        ))
    }));

    let mut query = world.term_query::<(&Velocity, &mut Position)>();

    for (velocity, mut position) in query.iter_mut(&mut world) {
        position.0 += velocity.0;
    }

    let mut total = 0.0;
    for (velocity, position) in query.iter(&mut world) {
        total += velocity.0.x + position.0.x;
    }

    dbg!(total);
}
