use std::collections::{
    hash_map,
    HashMap, HashSet,
};

use ggez::{
    GameResult, GameError,
};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use util::*;

// Epiphany: `Source` and `Sink` are *just* the input/output buffers.
// Sinks pull from available Sources until (has + incoming) >= need.
// Other behavior - production, reactor, etc. - are just inc/decs on
// the Source/Sink numbers.

#[derive(Debug)]
pub struct Source {
    pub count: usize,
}

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

impl Source {
    pub fn new() -> Self { Source { count: 0 } }
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