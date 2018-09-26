use std::collections::HashSet;

use ggez::GameResult;
use hex2d::{Coordinate, Direction, Spin};
use specs::{
    prelude::*,
    storage::BTreeStorage,
    Component,
};

use geom::*;
use util::*;

#[derive(Debug)]
pub struct Link {
    pub from: Entity,
    pub to: Entity,
    pub path: Vec<Coordinate>,
}

impl Component for Link {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Route {
    links: Vec<Entity /* Link */>,
    link_ix: usize,
    coord_ix: usize,
}

impl Component for Route {
    type Storage = BTreeStorage<Self>;
}

pub struct Traverse;

impl<'a> System<'a> for Traverse {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Link>,
        WriteStorage<'a, Motion>,
        WriteStorage<'a, Arrived>,
        WriteStorage<'a, Route>,
    );

    fn run(&mut self, (entities, links, mut motions, mut arrived, mut routes): Self::SystemData) {
        //let mut v = Vec::new();
        for (entity, motion, route, _) in (&*entities, &mut motions, &mut routes, &arrived).join() {
            let link = if let Some(l) = links.get(route.links[route.link_ix]) { l } else {
                // TODO: flag?
                continue;
            };
            route.coord_ix += 1;
            if route.coord_ix >= link.path.len() {
                route.coord_ix = 0;
                route.link_ix += 1;
                // TODO: update link variable
            }
            // TODO: clear arrived, reset motion, preserve motion overflow
        }
    }
}

const NODE_RADIUS: i32 = 1;

pub fn make_node(world: &mut World, center: Coordinate) -> Entity {
    world.create_entity()
        .with(Center(center))
        .with(Shape(center.ring(NODE_RADIUS, Spin::CW(Direction::XY))))
        .build()
}

pub fn make_link<'a>(world: &mut World, from: Entity, to: Entity) -> GameResult<Entity> {
    let mut path = vec![];
    let mut link_excl;
    {
        let centers = world.read_storage();
        let &Center(ref source_pos) = try_get(&centers, from)?;
        let &Center(ref sink_pos) = try_get(&centers, to)?;
        link_excl = HashSet::<Coordinate>::new();
        source_pos.for_each_in_range(NODE_RADIUS, |c| { link_excl.insert(c); });
        sink_pos.for_each_in_range(NODE_RADIUS, |c| { link_excl.insert(c); });
        source_pos.for_each_in_line_to(*sink_pos, |c| {
            if link_excl.contains(&c) { return };
            path.push(c);
        });
    }
    let ent = world.create_entity()
        .with(Shape(path.clone()))
        .with(Link { from, to, path })
        .build();
    Ok(ent)
}
