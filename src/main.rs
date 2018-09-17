extern crate specs;
extern crate hex2d;
extern crate mortal;

mod geom;
mod screen;

use hex2d::Coordinate;
use specs::{Builder, DispatcherBuilder, World};

fn main() {
    let mut world = World::new();

    const TRAVEL: &str = "travel";
    const DRAW_CELLS: &str = "draw_cells";

    let mut dispatcher = DispatcherBuilder::new()
        .with(geom::Travel, TRAVEL, &[])
        .with(geom::DrawCells, DRAW_CELLS, &[TRAVEL])
        .build();

    dispatcher.setup(&mut world.res);

    /*
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
    */
    Coordinate { x: 0, y: 0 }.for_each_in_ring(1, hex2d::Spin::CW(hex2d::Direction::XY), |coord| {
        world.create_entity()
            .with(geom::Cell(coord))
            .build();
    });

    let mut quit = false;
    while !quit {
        world.read_resource::<screen::Screen>().0.clear_screen();

        dispatcher.dispatch(&mut world.res);
        world.maintain();

        let screen = &world.read_resource::<screen::Screen>().0;
        screen.refresh().unwrap();
        let mut scr_read = screen.lock_read().unwrap();

        loop {
            let ev = if let Some(ev) = scr_read.read_event(None).unwrap() { ev } else { continue };
            use mortal::{Event::Key, Key::*};
            match ev {
                Key(Escape) => {
                    quit = true;
                    break;
                },
                Key(Char(' ')) => break,
                _ => ()
            };
        }
    }
}