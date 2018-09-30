use std::collections::{
    HashMap,
};

use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use geom;
use graph;
use util::*;

// Epiphany: `Source` and `Sink` are *just* the input/output buffers.
// Sinks pull from available Sources until (has + incoming) >= need.
// Other behavior - production, reactor, etc. - are just inc/decs on
// the Source/Sink numbers.

#[derive(Debug)]
pub struct Source {
    pub has: usize,
}

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

impl Source {
    pub fn new() -> Self { Source { has: 0 } }
}

#[derive(Debug)]
pub struct Sink {
    pub want: usize,
    pub count: usize,
    pub in_transit: usize,
    pub sources: HashMap<Entity /* Source */, Vec<Entity /* Node */>>,
}

impl Component for Sink {
    type Storage = BTreeStorage<Self>;
}

impl Sink {
    pub fn new(want: usize) -> Self {
        Sink {
            want, count: 0, in_transit: 0,
            sources: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Packet;

impl Component for Packet {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Pull;

impl<'a> System<'a> for Pull {
    type SystemData = (
        ReadStorage<'a, graph::Link>,
        ReadStorage<'a, graph::Node>,
        WriteStorage<'a, geom::Motion>,
        WriteStorage<'a, graph::Route>,
        WriteStorage<'a, Source>,
        WriteStorage<'a, Sink>,
    );

    fn run(&mut self, (links, nodes, mut motions, mut routes, mut sources, mut sinks): Self::SystemData) {
        for sink in sinks.join() {
            if sink.count + sink.in_transit >= sink.want { continue }
            let mut candidates: Vec<(usize, Entity)> = vec![];
            for (source_ent, route) in &sink.sources {
                let source = try_get_mut(&mut sources, *source_ent).unwrap();
                if source.has == 0 { continue }
                candidates.push((
                    graph::route_len(route, &links, &nodes).unwrap(),
                    *source_ent));
            }
            candidates.sort_unstable();
            // TODO: start packet from closest candidate, decrement source.has, increment sink.in_transit
        }
    }
}