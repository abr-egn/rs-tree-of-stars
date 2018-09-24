use std::collections::{
    HashMap,
    hash_map::Entry,
};

use ggez::{GameResult, GameError};
use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
    Component,
};

#[derive(Debug)]
pub struct Shape(pub Vec<Coordinate>);

impl Component for Shape {
    type Storage = VecStorage<Self>;
}

type Routes = HashMap<Entity /* Source/Sink */, Vec<Entity /* Link */>>;

#[derive(Debug)]
pub struct Source {
    pub sinks: Routes,
}

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Sink {
    pub sources: Routes,
}

impl Component for Sink {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]

pub struct Link {
    pub source: Entity,
    pub sink: Entity,
    pub path: Vec<Coordinate>,  // source -> sink
}

impl Component for Link {
    type Storage = BTreeStorage<Self>;
}

fn try_get_mut<'a, 'b, T: Component>(storage: &'b mut WriteStorage<'a, T>, ent: Entity) -> GameResult<&'b mut T> {
    match storage.get_mut(ent) {
        Some(t) => Ok(t),
        None => Err(GameError::UnknownError("no such entity".into())),
    }
}

pub fn connect<'a>(
    sources: WriteStorage<'a, Source>,
    sinks: WriteStorage<'a, Sink>,
    source: Entity,
    sink: Entity,
    route: &[Entity])
    -> GameResult<()> {
    let mut sources = sources;
    let mut sinks = sinks;

    match (try_get_mut(&mut sources, source)?.sinks.entry(sink), try_get_mut(&mut sinks, sink)?.sources.entry(source)) {
        (Entry::Vacant(source_route), Entry::Vacant(sink_route)) => {
            source_route.insert(route.iter().cloned().collect());
            sink_route.insert(route.iter().rev().cloned().collect());
        }
        _ => return Err(GameError::UnknownError("link already exists".into())),
    };

    Ok(())
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