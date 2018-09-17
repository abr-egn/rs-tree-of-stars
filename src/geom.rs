use hex2d::Coordinate;
use specs::{Component, Entities, System, Read, ReadStorage, Join, WriteStorage, VecStorage};
use specs::storage::BTreeStorage;

use screen;

/** Location **/

#[derive(Debug)]
pub struct Cell(pub Coordinate);

impl Component for Cell {
    type Storage = VecStorage<Self>;
}

pub struct PrintCells;

impl<'a> System<'a> for PrintCells {
    type SystemData = (
        Read<'a, screen::Screen>,
        ReadStorage<'a, Cell>,
    );

    fn run(&mut self, (screen, cells): Self::SystemData) {
        let mut ix = 0;
        for cell in cells.join() {
            screen.0.write_at((ix, 0), &format!("Coord: {:?}", cell.0));
            ix += 1;
        }
    }
}

/** Movement **/

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
        Entities<'a>,
        ReadStorage<'a, Speed>,
        WriteStorage<'a, Cell>,
        WriteStorage<'a, Path>,
    );

    fn run(&mut self, (entities, speed, mut cell, mut path): Self::SystemData) {
        let mut done = vec![];
        for (entity, speed, path, cell) in (&*entities, &speed, &mut path, &mut cell).join() {
            path.to_next += speed.0;
            if path.to_next >= 1.0 {
                path.to_next -= 1.0;
                path.index += 1;
                cell.0 = path.route[path.index];
                if path.index == path.route.len()-1 {
                    done.push(entity);
                }
            }
        }
        for e in done {
            path.remove(e);
        }
    }
}