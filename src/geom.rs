use std::collections::{
    HashMap, HashSet,
};

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
pub struct Packet {
    pub route: Vec<Entity /* Link */>,
    pub speed: f32,
    pub route_index: usize,
    pub path_index: usize,
    pub to_next: f32,
}

impl Packet {
    pub fn new(route: &[Entity], speed: f32) -> Self {
        Packet {
            route: route.to_owned(),
            speed: speed,
            route_index: 0,
            path_index: 0,
            to_next: 0.0,
        }
    }

    pub fn done(&self) -> bool { self.route_index >= self.route.len() }
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
        let delta = 1.0 / (super::UPDATES_PER_SECOND as f32);
        for packet in (&mut packets).join() {
            if packet.done() { continue };
            packet.to_next += packet.speed * delta;
            if packet.to_next >= 1.0 {
                packet.to_next -= 1.0;
                packet.path_index += 1;
                let link = if let Some(l) = links.get(packet.route[packet.route_index]) { l } else { continue };
                if packet.path_index >= link.path.len() {
                    packet.path_index = 0;
                    packet.route_index += 1;
                }
            }
        }
    }
}