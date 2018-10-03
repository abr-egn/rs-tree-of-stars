use std::collections::HashMap;

use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

#[derive(Debug)]
pub struct Location(Coordinate);

impl Location {
    pub fn new(coord: Coordinate) -> Self { Location(coord) }
    pub fn coord(&self) -> Coordinate { self.0 }
}

impl Component for Location {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Map(HashMap<Coordinate, Entity>);

impl Map {
    pub fn new() -> Self { Map(HashMap::new()) }
    #[allow(unused)]
    pub fn get(&self, coord: Coordinate) -> Option<&Entity> { self.0.get(&coord) }
    pub fn set(
        &mut self, locs: &mut WriteStorage<Location>,
        coord: Coordinate, ent: Entity,
    ) -> Option<Entity> {
        let old = self.0.insert(coord, ent)
            .map(|e| { locs.remove(e); e });
        locs.insert(ent, Location::new(coord)).unwrap();
        old
    }
    #[allow(unused)]
    pub fn clear(
        &mut self, locs: &mut WriteStorage<Location>,
        coord: Coordinate,
    ) -> Option<Entity> {
        self.0.remove(&coord)
            .map(|e| { locs.remove(e); e })
    }
    pub fn in_range(&self, center: Coordinate, radius: i32) -> Vec<Entity> {
        let mut out = vec![];
        center.for_each_in_range(radius, |c| {
            if let Some(&e) = self.0.get(&c) {
                out.push(e);
            }
        });
        out
    }
}