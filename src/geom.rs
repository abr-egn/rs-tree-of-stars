use std::collections::HashMap;

use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

#[derive(Debug)]
pub struct Shape(pub Vec<Coordinate>);

impl Component for Shape {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
pub struct Source;

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Sink {
    pub sources: HashMap<Entity /* source */, Vec<Entity /* Link */>>,
}

impl Component for Sink {
    type Storage = BTreeStorage<Self>;
}

/*

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
        ReadStorage<'a, Speed>,
        WriteStorage<'a, Cell>,
        WriteStorage<'a, Path>,
    );

    fn run(&mut self, (speed, mut cell, mut path): Self::SystemData) {
        let delta = 1.0 / (super::UPDATES_PER_SECOND as f32);
        for (speed, path, cell) in (&speed, &mut path, &mut cell).join() {
            path.to_next += speed.0 * delta;
            if path.to_next >= 1.0 {
                path.to_next -= 1.0;
                path.index = (path.index + 1) % path.route.len();
                cell.0 = path.route[path.index];
            }
        }
    }
}

*/