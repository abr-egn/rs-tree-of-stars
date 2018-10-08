use std::collections::{HashMap, HashSet};

use ggez::{
    nalgebra,
    graphics::Point2,
    GameResult, GameError,
};
use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use draw;
use util::*;

#[derive(Debug)]
pub struct Motion {
    pub from: Point2,
    pub to: Point2,
    pub inc: f32,
    pub at: f32,
}

impl Motion {
    pub fn new(from: Coordinate, to: Coordinate, speed: f32) -> Self {
        let (fx, fy) = from.to_pixel(draw::SPACING);
        let (tx, ty) = to.to_pixel(draw::SPACING);
        let from = Point2::new(fx, fy);
        let to = Point2::new(tx, ty);
        let dist = nalgebra::distance(&from, &to);
        /* Hex center to hex center is 2 * altitude of equilateral triangle */
        let speed_scale = 3.0f32.sqrt() * draw::HEX_SIDE;
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
pub struct Space(HashSet<Coordinate>);

impl Space {
    pub fn new<T>(coords: T) -> Self
        where T: IntoIterator<Item=Coordinate>,
    { Space(coords.into_iter().collect()) }
    pub fn coords(&self) -> &HashSet<Coordinate> { &self.0 }
}

impl Component for Space {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Map(HashMap<Coordinate, Entity>);

impl Map {
    pub fn new() -> Self { Map(HashMap::new()) }
    pub fn get(&self, coord: Coordinate) -> Option<Entity> { self.0.get(&coord).cloned() }
    pub fn is_occupied(&self, space: &Space) -> bool {
        space.coords().iter().any(|c| self.0.get(c).is_some())
    }
    pub fn set(
        &mut self, locs: &mut WriteStorage<Space>,
        ent: Entity, space: Space,
    ) -> GameResult<()> {
        if self.is_occupied(&space) {
            return Err(GameError::UnknownError(format!("occupied space: {:?}", space)))
        }
        for &c in space.coords() { self.0.insert(c, ent); }
        locs.insert(ent, space).unwrap();
        Ok(())
    }
    #[allow(unused)]
    pub fn clear(
        &mut self, locs: &mut WriteStorage<Space>,
        ent: Entity,
    ) -> GameResult<()> {
        {
            let space = try_get_mut(locs, ent)?;
            for c in space.coords() { self.0.remove(c); }
        }
        locs.remove(ent);
        Ok(())
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