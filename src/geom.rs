use std::collections::{
    HashMap, HashSet,
};

use ggez::{
    nalgebra,
    GameResult, GameError,
};
use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
    Component,
};

use game;

type Point = nalgebra::Point2<f32>;

#[derive(Debug)]
pub struct Shape(pub Vec<Coordinate>);

impl Component for Shape {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
pub struct Center(pub Coordinate);

impl Component for Center {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Source {
    pub sinks: HashMap<Entity /* Sink */, Vec<Entity /* Link */>>,
}

impl Source {
    pub fn new() -> Self { Source { sinks: HashMap::new() } }
}

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Sink {
    pub sources: HashSet<Entity /* Source */>,
}

impl Sink {
    pub fn new() -> Self { Sink { sources: HashSet::new() } }
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

#[derive(Debug)]
pub struct Motion {
    pub from: Point,
    pub to: Point,
    pub dist: f32,
    pub speed: f32,
    pub at: f32,
}

impl Motion {
    pub fn new(from: Coordinate, to: Coordinate, speed: f32) -> Self {
        let (fx, fy) = from.to_pixel(super::SPACING);
        let (tx, ty) = to.to_pixel(super::SPACING);
        let from = Point::new(fx, fy);
        let to = Point::new(tx, ty);
        let dist = nalgebra::distance(&from, &to);
        Motion { from, to, dist, speed, at: 0.0 }
    }
}

impl Component for Motion {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        WriteStorage<'a, Motion>,
    );

    fn run(&mut self, (mut motions, ): Self::SystemData) {
        for motion in (&mut motions).join() {
            if motion.at >= 1.0 { continue };
            motion.at += (motion.speed * super::UPDATE_DELTA) / motion.dist;
        }
    }
}

/*
#[derive(Debug)]
pub struct Packet {
    route: Vec<Entity /* Link */>,
    speed: f32,
    route_index: usize,
    path_index: usize,
    to_next: f32,
    from_hex: Coordinate,
    to_hex: Coordinate,
}

impl Packet {
    pub fn new<'a>(
        sources: &ReadStorage<'a, Source>,
        centers: &ReadStorage<'a, Center>,
        links: &ReadStorage<'a, Link>,
        source: Entity, sink: Entity, speed: f32
        ) -> GameResult<Self> {
        let source_val = game::try_get(sources, source)?;
        let route = if let Some(r) = source_val.sinks.get(&sink) { r } else {
            return Err(GameError::UnknownError("no route to sink".into()));
        };
        /*
        Packet {
            route: route.to_owned(),
            speed: speed,
            route_index: 0,
            path_index: 0,
            to_next: 0.0,
        }
        */
        unimplemented!()
    }

    pub fn done(&self) -> bool { self.route_index >= self.route.len() }

    fn update<'a>(&mut self, links: &ReadStorage<'a, Link>) {
        if self.done() { return };
        self.to_next += self.speed * super::UPDATE_DELTA;  // TODO: speed / distance
        if self.to_next >= 1.0 {
            self.to_next -= 1.0;
            self.path_index += 1;
            let link = if let Some(l) = links.get(self.route[self.route_index]) { l } else { return };
            if self.path_index >= link.path.len() {
                self.path_index = 0;
                self.route_index += 1;
            }
        }
    }
}

impl Component for Packet {
    type Storage = BTreeStorage<Self>;
}

pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        ReadStorage<'a, Link>,
        WriteStorage<'a, Packet>,
    );

    fn run(&mut self, (links, mut packets): Self::SystemData) {
        for packet in (&mut packets).join() {
            packet.update(&links);
        }
    }
}
*/