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
pub struct Shape(pub Vec<Coordinate>);

impl Component for Shape {
    type Storage = VecStorage<Self>;
}

#[derive(Debug, Copy, Clone)]
pub struct Center(pub Coordinate);

impl Component for Center {
    type Storage = FlaggedStorage<Self, BTreeStorage<Self>>;
}

#[derive(Debug)]
pub struct Map(HashMap<Coordinate, Entity>);

impl Map {
    pub fn new() -> Self { Map(HashMap::new()) }

    fn get(&self, coord: &Coordinate, centers: &ReadStorage<Center>) -> Option<(Entity, Center)> {
        let ent = if let Some(&e) = self.0.get(coord) { e } else { return None };
        match centers.get(ent) {
            None => None,  // TODO: flag for cleanup
            Some(c) => Some((ent, *c))
        }
    }
}

#[derive(Debug)]
pub struct MapUpdate {
    inserted: ReaderId<InsertedFlag>,
    modified: ReaderId<ModifiedFlag>,
    removed: ReaderId<RemovedFlag>,
}

impl MapUpdate {
    pub fn new(centers: &mut WriteStorage<Center>) -> Self {
        MapUpdate {
            inserted: centers.track_inserted(),
            modified: centers.track_modified(),
            removed: centers.track_removed(),
        }
    }
}

impl<'a> System<'a> for MapUpdate {
    type SystemData = (
        Entities<'a>,
        WriteExpect<'a, Map>,
        ReadStorage<'a, Center>,
    );

    fn run(&mut self, (entities, mut map, centers): Self::SystemData) {
        let mut dirty = BitSet::new();
        centers.populate_removed(&mut self.removed, &mut dirty);
        // TODO: problem: when it's removed, the coordinate no longer exists to remove
        // from the map.
        /*
        for (entity, _) in (&*entities, &dirty).join() {
            map.0.remove(&entity);
        }
        */
    }
}

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