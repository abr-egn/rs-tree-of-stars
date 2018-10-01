use std::collections::{
    HashSet, HashMap,
};

use ggez::{GameResult, GameError};
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
pub struct Node {
    pub links_to: HashMap<Entity /* Node */, Entity /* Link */>,
    pub links_from: HashMap<Entity /* Node */, Entity /* Link */>,
}

impl Node {
    pub fn new() -> Self {
        Node {
            links_to: HashMap::new(),
            links_from: HashMap::new(),
        }
    }
}

impl Component for Node {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
enum PathDir {
    Fwd,
    Rev,
}

fn try_get_link<'a>(
    from_ent: Entity, to_ent: Entity,
    links: &'a ReadStorage<Link>, nodes: &ReadStorage<Node>)
    -> GameResult<Option<(&'a Link, PathDir)>> {
    let from = try_get(&nodes, from_ent)?;
    if let Some(&link_ent) = from.links_to.get(&to_ent) {
        let link = try_get(&links, link_ent)?;
        Ok(Some((link, PathDir::Fwd)))
    } else {
        if let Some(&link_ent) = from.links_from.get(&to_ent) {
            let link = try_get(&links, link_ent)?;
            Ok(Some((link, PathDir::Rev)))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug)]
enum PathCoord {
    More,
    End,
}

fn path_ix(
    from_ent: Entity, to_ent: Entity, ix: usize,
    links: &ReadStorage<Link>, nodes: &ReadStorage<Node>)
    -> GameResult<(Coordinate, PathCoord)> {
    let (link, coord_ix) = match try_get_link(from_ent, to_ent, links, nodes)? {
        None => return Err(GameError::UnknownError("invalid path".into())),
        Some((link, PathDir::Fwd)) => (link, ix),
        Some((link, PathDir::Rev)) => (link, link.path.len() - 1 - ix),
    };
    if ix >= link.path.len() {
        return Err(GameError::UnknownError("path ix past the end".into()))
    }
    Ok((
        link.path[coord_ix], 
        if ix == link.path.len()-1 { PathCoord::End } else { PathCoord::More }
    ))
}

fn path_len(
    from_ent: Entity, to_ent: Entity,
    links: &ReadStorage<Link>, nodes: &ReadStorage<Node>)
    -> GameResult<usize> {
    let (link, _) = try_get_link(from_ent, to_ent, links, nodes)?
        .ok_or(GameError::UnknownError("invalid path".into()))?;
    Ok(link.path.len())
}

pub fn route_len(
    route: &[Entity],
    links: &ReadStorage<Link>, nodes: &ReadStorage<Node>)
    -> GameResult<usize> {
    let mut total: usize = 0;
    for ix in 0..route.len()-1 {
        total += path_len(route[ix], route[ix+1], links, nodes)?;
    }

    Ok(total)
}

#[derive(Debug)]
pub struct Route {
    nodes: Vec<Entity /* Node */>,
    speed: f32,
    node_ix: usize,
    coord_ix: usize,
}

impl Route {
    fn new(nodes: &[Entity], speed: f32) -> Self {
        let nodes = nodes.into();
        Route { nodes, speed, node_ix: 0, coord_ix: 0 }
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

impl Traverse {
    pub fn start(
        entity: Entity,
        start: Coordinate,
        route_nodes: &[Entity],
        speed: f32,
        links: &ReadStorage<Link>,
        nodes: &ReadStorage<Node>,
        motions: &mut WriteStorage<Motion>,
        routes: &mut WriteStorage<Route>)
        -> GameResult<()> {
        let (first_coord, _) = path_ix(route_nodes[0], route_nodes[1], 0, &links, &nodes)?;
        let route = Route::new(route_nodes, speed);
        motions.insert(entity, Motion::new(start, first_coord, route.speed)).map_err(dbg)?;
        routes.insert(entity, route).map_err(dbg)?;
        Ok(())
    }
}

#[derive(SystemData)]
pub struct TraverseData<'a> {
    entities: Entities<'a>,
    links: ReadStorage<'a, Link>,
    nodes: ReadStorage<'a, Node>,
    motions: WriteStorage<'a, Motion>,
    motion_done: WriteStorage<'a, MotionDone>,
    routes: WriteStorage<'a, Route>,
    route_done: WriteStorage<'a, RouteDone>,
}

impl<'a> System<'a> for Traverse {
    type SystemData = TraverseData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        let mut more_motion = Vec::new();
        let mut no_more_route = Vec::new();
        for (entity, motion, route, _, ()) in (
            &*data.entities, &mut data.motions, &mut data.routes,
            &data.motion_done, !&data.route_done).join() {
            let (from_coord, more) = path_ix(
                route.nodes[route.node_ix], route.nodes[route.node_ix+1], route.coord_ix,
                &data.links, &data.nodes)
                .unwrap();
            match more {
                PathCoord::More => {
                    route.coord_ix += 1;
                },
                PathCoord::End => {
                    route.coord_ix = 0;
                    route.node_ix += 1;
                    if route.node_ix >= route.nodes.len()-1 {
                        no_more_route.push(entity);
                        continue;
                    }
                },
            }
            let (to_coord, _) = path_ix(
                route.nodes[route.node_ix], route.nodes[route.node_ix+1], route.coord_ix,
                &data.links, &data.nodes)
                .unwrap();
            more_motion.push(entity);  // arrival flag clear
            let rem = motion.at - 1.0;
            *motion = Motion::new(from_coord, to_coord, route.speed);
            motion.at = rem;
        }
        for entity in more_motion {
            data.motion_done.remove(entity);
        }
        for entity in no_more_route {
            data.route_done.insert(entity, RouteDone).unwrap();
        }
    }
}

const NODE_RADIUS: i32 = 1;

pub fn make_node(world: &mut World, center: Coordinate) -> Entity {
    world.create_entity()
        .with(Node::new())
        .with(Center(center))
        .with(Shape(center.ring(NODE_RADIUS, Spin::CW(Direction::XY))))
        .build()
}

pub fn make_link(world: &mut World, from: Entity, to: Entity) -> GameResult<Entity> {
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
    let mut nodes = world.write_storage::<Node>();
    {
        let from_node = try_get_mut(&mut nodes, from)?;
        from_node.links_to.insert(to, ent);
    }
    {
        let to_node = try_get_mut(&mut nodes, to)?;
        to_node.links_from.insert(from, ent);
    }
    Ok(ent)
}
