use std::collections::{
    hash_map::Entry,
    HashSet,
};

use ggez::{GameResult, GameError};
use hex2d::{Coordinate, Direction, Spin};
use specs::prelude::*;

use geom::{Center, Link, Shape, Sink, Source};

const NODE_RADIUS: i32 = 1;

pub fn make_node(world: &mut World, center: Coordinate) -> Entity {
    world.create_entity()
        .with(Center(center))
        .with(Shape(center.ring(NODE_RADIUS, Spin::CW(Direction::XY))))
        .build()
}

fn try_get<'a, 'b, T: Component>(storage: &'b ReadStorage<'a, T>, ent: Entity) -> GameResult<&'b T> {
    match storage.get(ent) {
        Some(t) => Ok(t),
        None => Err(GameError::UnknownError("no such entity".into())),
    }
}

fn try_get_mut<'a, 'b, T: Component>(storage: &'b mut WriteStorage<'a, T>, ent: Entity) -> GameResult<&'b mut T> {
    match storage.get_mut(ent) {
        Some(t) => Ok(t),
        None => Err(GameError::UnknownError("no such entity".into())),
    }
}

pub fn make_link(world: &mut World, source: Entity, sink: Entity) -> GameResult<Entity> {
    let path;
    let mut link_excl;
    {
        let centers = world.read_storage::<Center>();
        let &Center(ref source_pos) = try_get(&centers, source)?;
        let &Center(ref sink_pos) = try_get(&centers, sink)?;
        path = source_pos.line_to(*sink_pos);
        link_excl = HashSet::<Coordinate>::new();
        source_pos.for_each_in_range(NODE_RADIUS, |c| { link_excl.insert(c); });
        sink_pos.for_each_in_range(NODE_RADIUS, |c| { link_excl.insert(c); });
    }
    let ent = world.create_entity()
        .with(Shape(path.iter().cloned()
                .filter(|c| !link_excl.contains(c))
                .collect()))
        .with(Link { source, sink, path })
        .build();
    Ok(ent)
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
        (Entry::Vacant(source_route), false) => {
            source_route.insert(route.iter().cloned().collect());
            sink_sources.insert(source);
        }
        _ => return Err(GameError::UnknownError("link already exists".into())),
    };

    Ok(())
}
