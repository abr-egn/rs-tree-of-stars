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

pub fn connect<'a>(
    sources: WriteStorage<'a, Source>,
    sinks: WriteStorage<'a, Sink>,
    source: Entity,
    sink: Entity,
    route: &[Entity])
    -> GameResult<()> {
    let mut sources = sources;
    let mut sinks = sinks;

    let sink_sources = &mut try_get_mut(&mut sinks, sink)?.sources;
    match (try_get_mut(&mut sources, source)?.sinks.entry(sink), sink_sources.contains(&source)) {
        (hash_map::Entry::Vacant(source_route), false) => {
            source_route.insert(route.iter().cloned().collect());
            sink_sources.insert(source);
        }
        _ => return Err(GameError::UnknownError("link already exists".into())),
    };

    Ok(())
}