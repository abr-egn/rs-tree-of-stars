use hex2d::{self, Coordinate};
use specs::{
    storage::BTreeStorage,
    Component, Entities, System, Read, ReadStorage, Join, WriteStorage, VecStorage
};
use mortal;

use screen::Screen;

/** Location **/

#[derive(Debug)]
pub struct Cell(pub Coordinate);

impl Component for Cell {
    type Storage = VecStorage<Self>;
}

pub struct DrawCells;

const SPACING: hex2d::IntegerSpacing<i32> = hex2d::IntegerSpacing::FlatTop(3, 2);

impl<'a> System<'a> for DrawCells {
    type SystemData = (
        Read<'a, Screen>,
        ReadStorage<'a, Cell>,
    );

    fn run(&mut self, (screen, cells): Self::SystemData) {
        let screen = &screen.0;
        let mortal::Size { lines, columns } = screen.size();
        let line_off = (lines / 2) as i32;
        let col_off = (columns / 2) as i32;
        for Cell(coord) in cells.join() {
            let (x, y) = coord.to_pixel_integer(SPACING);
            let line = (y + line_off) as usize;
            let col = (x + col_off) as usize;
            screen.write_at((line, col), "*");
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