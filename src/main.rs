extern crate specs;

use specs::{Builder, Component, DispatcherBuilder, System, Read, ReadStorage, Join, VecStorage, World, WriteStorage};

#[derive(Debug)]
struct Position {
    x: f32,
    y: f32,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
struct Velocity {
    x: f32,
    y: f32,
}

impl Component for Velocity {
    type Storage = VecStorage<Self>;
}

struct HelloWorld;

impl<'a> System<'a> for HelloWorld {
    type SystemData = ReadStorage<'a, Position>;

    fn run(&mut self, position: Self::SystemData) {
        for position in position.join() {
            println!("Hello, {:?}", &position);
        }
    }
}

struct UpdatePos;

impl<'a> System<'a> for UpdatePos {
    type SystemData = (Read<'a, DeltaTime>,
                       ReadStorage<'a, Velocity>,
                       WriteStorage<'a, Position>);

    fn run(&mut self, (delta, vel, mut pos): Self::SystemData) {
        for (vel, pos) in (&vel, &mut pos).join() {
            pos.x += vel.x * delta.0;
            pos.y += vel.y * delta.0;
        }
    }
}

#[derive(Default)]
struct DeltaTime(f32);

fn main() {
    let mut world = World::new();
    world.register::<Position>();
    world.register::<Velocity>();
    world.add_resource(DeltaTime(0.05));
    {
        let mut delta = world.write_resource::<DeltaTime>();
        *delta = DeltaTime(0.04);
    }

    world.create_entity().with(Position { x: 4.0, y: 7.0 }).build();
    world.create_entity()
        .with(Position { x: 2.0, y: 5.0 })
        .with(Velocity { x: 0.1, y: 0.2 })
        .build();

    const HELLO_WORLD: &str = "hello_world";
    const UPDATE_POS: &str = "update_pos";
    const HELLO_UPDATED: &str = "hello_updated";

    let mut dispatcher = DispatcherBuilder::new()
        .with(HelloWorld, HELLO_WORLD, &[])
        .with(UpdatePos, UPDATE_POS, &[HELLO_WORLD])
        .with(HelloWorld, HELLO_UPDATED, &[UPDATE_POS])
        .build();

    dispatcher.dispatch(&mut world.res);
    world.maintain();
}
