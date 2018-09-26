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
    speed: f32,
    link_ix: usize,
    coord_ix: usize,
}

impl Route {
    pub fn new(links: &[Entity], speed: f32) -> Self {
        let links = links.into();
        Route { links, speed, link_ix: 0, coord_ix: 0 }
    }

    pub fn start(
        entity: Entity,
        start: Coordinate,
        route: Route,
        links: ReadStorage<Link>,
        mut motions: WriteStorage<Motion>,
        mut routes: WriteStorage<Route>)
        -> GameResult<()> {
        let link = try_get(&links, route.links[route.link_ix])?;
        motions.insert(entity, Motion::new(start, link.path[route.coord_ix], route.speed)).map_err(dbg)?;
        routes.insert(entity, route).map_err(dbg)?;
        Ok(())
    }
}

impl Component for Route {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Default)]
pub struct RouteDone;

impl Component for RouteDone {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct Traverse;

impl<'a> System<'a> for Traverse {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Link>,
        WriteStorage<'a, Motion>,
        WriteStorage<'a, MotionDone>,
        WriteStorage<'a, Route>,
        WriteStorage<'a, RouteDone>,
    );

    fn run(&mut self, (entities, links, mut motions, mut motions_done, mut routes, mut routes_done): Self::SystemData) {
        let mut more_motion = Vec::new();
        let mut no_more_route = Vec::new();
        for (entity, motion, route, _, ()) in (&*entities, &mut motions, &mut routes, &motions_done, !&routes_done).join() {
            let mut link = if let Some(l) = links.get(route.links[route.link_ix]) { l } else {
                // TODO: flag?
                continue;
            };
            let from_coord = link.path[route.coord_ix];
            route.coord_ix += 1;
            if route.coord_ix >= link.path.len() {
                route.coord_ix = 0;
                route.link_ix += 1;
                if route.link_ix >= route.links.len() {
                    no_more_route.push(entity);
                    continue;
                }
                // TODO: factor out link-lookup somehow?
                link = if let Some(l) = links.get(route.links[route.link_ix]) { l } else {
                    // TODO: flag?
                    continue;
                };
            }
            let to_coord = link.path[route.coord_ix];
            more_motion.push(entity);  // arrival flag clear
            let rem = motion.at - 1.0;
            *motion = Motion::new(from_coord, to_coord, route.speed);
            motion.at = rem;
        }
        for entity in more_motion {
            motions_done.remove(entity);
        }
        for entity in no_more_route {
            routes_done.insert(entity, RouteDone).unwrap();
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
