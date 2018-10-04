use std::collections::HashMap;

use ggez::{
    nalgebra,
    graphics::Point2,
};
use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

#[derive(Debug)]
pub struct Motion {
    pub from: Point2,
    pub to: Point2,
    pub inc: f32,
    pub at: f32,
}

impl Motion {
    pub fn new(from: Coordinate, to: Coordinate, speed: f32) -> Self {
        let (fx, fy) = from.to_pixel(super::SPACING);
        let (tx, ty) = to.to_pixel(super::SPACING);
        let from = Point2::new(fx, fy);
        let to = Point2::new(tx, ty);
        let dist = nalgebra::distance(&from, &to);
        /* Hex center to hex center is 2 * altitude of equilateral triangle */
        let speed_scale = 3.0f32.sqrt() * super::HEX_SIDE;
        let inc = (speed * speed_scale * super::UPDATE_DELTA) / dist;
        Motion { from, to, inc, at: 0.0 }
    }
}

impl Component for Motion {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Default)]
pub struct MotionDone;

impl Component for MotionDone {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, Motion>,
        WriteStorage<'a, MotionDone>,
    );

    fn run(&mut self, (entities, mut motions, mut arrived): Self::SystemData) {
        let mut v = Vec::new();
        for (entity, motion, ()) in (&*entities, &mut motions, !&arrived).join() {
            if motion.at >= 1.0 { continue };
            motion.at += motion.inc;
            if motion.at >= 1.0 {
                v.push(entity);
            }
        }
        for entity in v {
            arrived.insert(entity, MotionDone).unwrap();
        }
    }
}

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