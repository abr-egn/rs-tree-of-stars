extern crate specs;
extern crate hex2d;

mod geom;

use hex2d::Coordinate;
use specs::{Builder, DispatcherBuilder, World};

fn main() {
    let mut world = World::new();

    const TRAVEL: &str = "travel";
    const PRINT_CELLS: &str = "print_cells";

    let mut dispatcher = DispatcherBuilder::new()
        .with(geom::Travel, TRAVEL, &[])
        .with(geom::PrintCells, PRINT_CELLS, &[TRAVEL])
        .build();

    dispatcher.setup(&mut world.res);

    world.create_entity()
        .with(geom::Cell(Coordinate { x: 0, y: 0 }))
        .build();
    world.create_entity()
        .with(geom::Cell(Coordinate { x: 1, y: 1 }))
        .with(geom::Speed(0.5))
        .with(geom::Path {
            route: vec![Coordinate { x: 1, y: 1 }, Coordinate { x: 2, y: 2}],
            index: 0,
            to_next: 0.0,
        })
        .build();

    for _ in 1..=3 {
        dispatcher.dispatch(&mut world.res);
        world.maintain();
    }
}