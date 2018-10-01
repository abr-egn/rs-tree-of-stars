use std::collections::{
    HashSet,
};

use ggez::{GameResult, GameError};
use hex2d::{Coordinate, Direction, Spin};
use petgraph::{
    self,
    graphmap::GraphMap,
};
use specs::{
    prelude::*,
    storage::BTreeStorage,
    Component,
};

use geom::*;
use util::*;

pub struct Graph(GraphMap<Entity, Entity, petgraph::Undirected>);

impl Graph {
    pub fn new() -> Self { Graph(GraphMap::new()) }
}

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
enum PathDir {
    Fwd,
    Rev,
}

fn try_get_link<'a>(
    from_ent: Entity, to_ent: Entity,
    graph: &Graph,
    links: &'a ReadStorage<Link>,
) -> GameResult<Option<(&'a Link, PathDir)>> {
    let link = if let Some(&link_ent) = graph.0.edge_weight(from_ent, to_ent) {
        try_get(&links, link_ent)?
    } else {
        return Ok(None)
    };
    let dir = if link.from == from_ent {
        PathDir::Fwd
    } else if link.to == from_ent {
        PathDir::Rev
    } else {
        panic!("Invalid link data")
    };
    Ok(Some((link, dir)))
}

#[derive(Debug, Copy, Clone)]
enum PathCoord {
    More,
    End,
}

fn path_ix(
    from_ent: Entity, to_ent: Entity, ix: usize,
    graph: &Graph,
    links: &ReadStorage<Link>
) -> GameResult<(Coordinate, PathCoord)> {
    let (link, coord_ix) = match try_get_link(from_ent, to_ent, graph, links)? {
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
    graph: &Graph,
    links: &ReadStorage<Link>,
) -> GameResult<usize> {
    let (link, _) = try_get_link(from_ent, to_ent, graph, links)?
        .ok_or(GameError::UnknownError("invalid path".into()))?;
    Ok(link.path.len())
}

pub fn route_len(
    route: &[Entity],
    graph: &Graph,
    links: &ReadStorage<Link>,
) -> GameResult<usize> {
    let mut total: usize = 0;
    for ix in 0..route.len()-1 {
        total += path_len(route[ix], route[ix+1], graph, links)?;
    }

    Ok(total)
}

#[derive(Debug)]
pub struct Route {
    nodes: Vec<Entity /* Node */>,
    speed: f32,
    node_ix: usize,
    coord_ix: usize,
    phase: RoutePhase,
}

#[derive(Debug, Copy, Clone)]
enum RoutePhase {
    ToLink(Coordinate, PathCoord),
    ToNode(Coordinate),
}

impl Route {
    fn new(nodes: &[Entity], speed: f32, phase: RoutePhase) -> Self {
        let nodes = nodes.into();
        Route { nodes, speed, node_ix: 0, coord_ix: 0, phase }
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
        graph: &Graph,
        links: &ReadStorage<Link>,
        motions: &mut WriteStorage<Motion>,
        routes: &mut WriteStorage<Route>,
    ) -> GameResult<()> {
        let (first_coord, p) = path_ix(route_nodes[0], route_nodes[1], 0, graph, links)?;
        let route = Route::new(route_nodes, speed, RoutePhase::ToLink(first_coord, p));
        motions.insert(entity, Motion::new(start, first_coord, route.speed)).map_err(dbg)?;
        routes.insert(entity, route).map_err(dbg)?;
        Ok(())
    }
}

#[derive(SystemData)]
pub struct TraverseData<'a> {
    entities: Entities<'a>,
    graph: ReadExpect<'a, Graph>,
    links: ReadStorage<'a, Link>,
    centers: ReadStorage<'a, Center>,
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
            /* Given the phase of motion that has finished,
                where is it now, and what's the next phase? */
            let (from_coord, link_next) = match route.phase {
                RoutePhase::ToLink(c, m) => {
                    let l = match m {
                        PathCoord::More => {
                            route.coord_ix += 1;
                            true
                        },
                        PathCoord::End => false,
                    };
                    (c, l)
                },
                RoutePhase::ToNode(c) => {
                    route.coord_ix = 0;
                    route.node_ix += 1;
                    if route.node_ix >= route.nodes.len()-1 {
                        no_more_route.push(entity);
                        continue
                    }
                    (c, true)
                },
            };
            /* And given the new phase, where is it going? */
            let to_coord = if link_next {
                let (coord, more) = path_ix(
                    route.nodes[route.node_ix], route.nodes[route.node_ix+1],
                    route.coord_ix,
                    &data.graph, &data.links
                ).unwrap();
                route.phase = RoutePhase::ToLink(coord, more);
                coord
            } else {
                let Center(coord) = try_get(&data.centers, route.nodes[route.node_ix+1]).unwrap();
                route.phase = RoutePhase::ToNode(*coord);
                *coord
            };
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
        .with(Center(center))
        .with(Shape(center.ring(NODE_RADIUS, Spin::CW(Direction::XY))))
        .build()
}

pub fn make_link(world: &mut World, from: Entity, to: Entity) -> GameResult<Entity> {
    let mut path = vec![];
    let mut shape = vec![];
    let mut shape_excl;
    {
        let centers = world.read_storage();
        let &Center(ref source_pos) = try_get(&centers, from)?;
        let &Center(ref sink_pos) = try_get(&centers, to)?;
        shape_excl = HashSet::<Coordinate>::new();
        source_pos.for_each_in_range(NODE_RADIUS, |c| { shape_excl.insert(c); });
        sink_pos.for_each_in_range(NODE_RADIUS, |c| { shape_excl.insert(c); });
        source_pos.for_each_in_line_to(*sink_pos, |c| {
            if !shape_excl.contains(&c) { shape.push(c); }
            if c != *source_pos && c != *sink_pos { path.push(c); }
        });
    }
    let ent = world.create_entity()
        .with(Shape(shape))
        .with(Link { from, to, path })
        .build();
    let mut graph = world.write_resource::<Graph>();
    graph.0.add_edge(from, to, ent);
    Ok(ent)
}
