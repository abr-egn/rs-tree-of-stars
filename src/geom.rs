use hex2d::Coordinate;
use specs::{Component, Entities, System, ReadStorage, Join, WriteStorage, VecStorage};
use specs::storage::BTreeStorage;

/** Location **/

#[derive(Debug)]
pub struct Cell(pub Coordinate);

impl Component for Cell {
    type Storage = VecStorage<Self>;
}

pub struct PrintCells;

impl<'a> System<'a> for PrintCells {
    type SystemData = ReadStorage<'a, Cell>;

    fn run(&mut self, cells: Self::SystemData) {
        for cell in cells.join() {
            println!("Coord: {:?}", cell.0);
        }
    }
}

/** Movement **/

#[derive(Debug)]
pub struct Path(pub Vec<Coordinate>);

impl Component for Path {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Speed(pub f32);

impl Component for Speed {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Progress {
    pub index: isize,
    pub to_next: f32,
}

impl Component for Progress {
    type Storage = BTreeStorage<Self>;
}

pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Path>,
        ReadStorage<'a, Speed>,
        WriteStorage<'a, Cell>,
        WriteStorage<'a, Progress>,
    );

    fn run(&mut self, (entities, path, speed, mut cell, mut progress): Self::SystemData) {
        for (entity, path, speed, cell) in (&*entities, &path, &speed, &mut cell).join() {
            let progress: &mut Progress = progress.entry(entity).unwrap().or_insert_with(|| Progress { index: 0, to_next: 0.0 });
        }
    }
}