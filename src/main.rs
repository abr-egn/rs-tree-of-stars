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

    dispatcher.dispatch(&mut world.res);
    world.maintain();
}