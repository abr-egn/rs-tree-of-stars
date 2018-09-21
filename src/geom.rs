use time::UpdateDelta;

use hex2d::{self, Coordinate};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

/* Location */

#[derive(Debug)]
pub struct Cell(pub Coordinate);

impl Component for Cell {
    type Storage = VecStorage<Self>;
}

/* Movement */

#[derive(Debug)]
pub struct Speed(pub f32);

impl Component for Speed {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Path {
    pub route: Vec<Coordinate>,
    pub index: usize,
    pub to_next: f32,
}

impl Component for Path {
    type Storage = BTreeStorage<Self>;
}

pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        Read<'a, UpdateDelta>,
        ReadStorage<'a, Speed>,
        WriteStorage<'a, Cell>,
        WriteStorage<'a, Path>,
    );

    fn run(&mut self, (delta, speed, mut cell, mut path): Self::SystemData) {
        for (speed, path, cell) in (&speed, &mut path, &mut cell).join() {
            path.to_next += speed.0 * delta.seconds();
            if path.to_next >= 1.0 {
                path.to_next -= 1.0;
                path.index = (path.index + 1) % path.route.len();
                cell.0 = path.route[path.index];
            }
        }
    }
}