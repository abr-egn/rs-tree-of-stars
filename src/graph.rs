use std::{
    cmp::max,
    collections::HashSet,
};

use ggez::{graphics, GameResult, GameError};
use hex2d::{Coordinate, Direction, Spin};
use petgraph::{
    self,
    graphmap::GraphMap,
};
use specs::{
    prelude::*,
    storage::{BTreeStorage, GenericReadStorage},
    Component,
};

use draw;
use geom;
use resource;
use util::*;

#[derive(Debug)]
pub struct Graph(GraphMap<Entity, Entity, petgraph::Undirected>);

pub type Route = Vec<(Entity, PathDir)>;

impl Graph {
    pub fn new() -> Self { Graph(GraphMap::new()) }
    pub fn add_link(&mut self, link: &Link, entity: Entity) {
        self.0.add_edge(link.from, link.to, entity);
    }
    pub fn route(
        &self, links: &ReadStorage<Link>, nodes: &ReadStorage<Node>,
        from: Entity, to: Entity) -> Option<(usize, Route)> {
        let from_coord = nodes.get(from).unwrap().at;
        let (len, nodes) = if let Some(p) = petgraph::algo::astar(
            /* graph= */ &self.0,
            /* start= */ from,
            /* is_goal= */ |ent| { ent == to },
            /* edge_cost= */ |(_, _, &link_ent)| {
                links.get(link_ent).unwrap().path.len()
            },
            /* estimate_cost= */ |ent| {
                let ent_coord = nodes.get(ent).unwrap().at;
                max(0, from_coord.distance(ent_coord) - 2) as usize
            },
        ) { p } else { return None };
        let mut route: Vec<(Entity, PathDir)> = vec![];
        for ix in 0..nodes.len()-1 {
            let link_ent = *self.0.edge_weight(nodes[ix], nodes[ix+1]).unwrap();
            let link = links.get(link_ent).unwrap();
            route.push((link_ent, if link.from == nodes[ix] {
                PathDir::Fwd
            } else if link.to == nodes[ix] {
                PathDir::Rev
            } else {
                panic!("invalid link data")
            }))
        }
        Some((len, route))
    }
}

#[derive(Debug, Copy, Clone)]
pub enum PathDir {
    Fwd,
    Rev,
}

#[derive(Debug)]
pub struct Node {
    at: Coordinate,
}

impl Node {
    pub fn at(&self) -> Coordinate { self.at }
}

impl Component for Node {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Clone)]
pub struct Link {
    pub from: Entity,
    pub to: Entity,
    pub path: Vec<Coordinate>,
}

impl Component for Link {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Copy, Clone)]
enum PathCoord {
    More,
    End,
}

fn path_ix(
    (link_ent, path_dir): (Entity, PathDir), ix: usize,
    links: &ReadStorage<Link>,
) -> GameResult<(Coordinate, PathCoord)> {
    let link = try_get(links, link_ent)?;
    let coord_ix = match path_dir {
        PathDir::Fwd => ix,
        PathDir::Rev => link.path.len() - 1 - ix,
    };
    if ix >= link.path.len() {
        return Err(GameError::UnknownError("path ix past the end".into()))
    }
    Ok((
        link.path[coord_ix], 
        if ix == link.path.len()-1 { PathCoord::End } else { PathCoord::More }
    ))
}

#[derive(Debug)]
pub struct FollowRoute {
    route: Route,
    speed: f32,
    link_ix: usize,
    coord_ix: usize,
    phase: RoutePhase,
}

#[derive(Debug, Copy, Clone)]
enum RoutePhase {
    ToLink(Coordinate, PathCoord),
    ToNode(Coordinate),
}

impl FollowRoute {
    fn new(route: Route, speed: f32, phase: RoutePhase) -> Self {
        FollowRoute { route, speed, link_ix: 0, coord_ix: 0, phase }
    }
}

impl Component for FollowRoute {
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
        route: Route,
        speed: f32,
        links: &ReadStorage<Link>,
        motions: &mut WriteStorage<geom::Motion>,
        routes: &mut WriteStorage<FollowRoute>,
    ) -> GameResult<()> {
        let (first_coord, p) = path_ix(route[0], 0, links)?;
        let follow = FollowRoute::new(route, speed, RoutePhase::ToLink(first_coord, p));
        motions.insert(entity, geom::Motion::new(start, first_coord, follow.speed)).map_err(dbg)?;
        routes.insert(entity, follow).map_err(dbg)?;
        Ok(())
    }
}

#[derive(SystemData)]
pub struct TraverseData<'a> {
    entities: Entities<'a>,
    links: ReadStorage<'a, Link>,
    nodes: ReadStorage<'a, Node>,
    motions: WriteStorage<'a, geom::Motion>,
    motion_done: WriteStorage<'a, geom::MotionDone>,
    routes: WriteStorage<'a, FollowRoute>,
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
                    route.link_ix += 1;
                    if route.link_ix >= route.route.len() {
                        no_more_route.push(entity);
                        continue
                    }
                    (c, true)
                },
            };
            /* And given the new phase, where is it going? */
            let to_coord = if link_next {
                let (coord, more) = path_ix(
                    route.route[route.link_ix],
                    route.coord_ix,
                    &data.links
                ).unwrap();
                route.phase = RoutePhase::ToLink(coord, more);
                coord
            } else {
                let link = data.links.get(route.route[route.link_ix].0).unwrap();
                let coord = data.nodes.get(link.to).unwrap().at;
                route.phase = RoutePhase::ToNode(coord);
                coord
            };
            more_motion.push(entity);  // arrival flag clear
            let rem = motion.at - 1.0;
            *motion = geom::Motion::new(from_coord, to_coord, route.speed);
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

pub fn node_shape(center: Coordinate) -> Vec<Coordinate> {
    center.ring(NODE_RADIUS, Spin::CW(Direction::XY))
}

pub fn node_space(center: Coordinate) -> Vec<Coordinate> {
    center.range(NODE_RADIUS)
}

pub fn space_for_node(map: &geom::Map, center: Coordinate) -> bool {
    for coord in node_space(center) {
        if map.get(coord).is_some() { return false }
    }
    true
}

pub fn make_node(
    entities: &Entities,
    map: &mut geom::Map,
    spaces: &mut WriteStorage<geom::Space>,
    shapes: &mut WriteStorage<draw::Shape>,
    nodes: &mut WriteStorage<Node>,
    center: Coordinate,
) -> GameResult<Entity> {
    let ent = entities.create();
    map.set(
        spaces, ent,
        geom::Space::new(node_space(center)),
    )?;
    shapes.insert(ent, draw::Shape {
        coords: node_shape(center),
        color: graphics::Color::new(0.8, 0.8, 0.8, 1.0),
    }).unwrap();
    nodes.insert(ent, Node { at: center }).unwrap();
    Ok(ent)
}

pub fn make_node_world(world: &mut World, center: Coordinate) -> GameResult<Entity> {
    make_node(
        &world.entities(),
        &mut world.write_resource(),
        &mut world.write_storage(),
        &mut world.write_storage(),
        &mut world.write_storage(),
        center,
    )
}

struct LinkSpace {
    from: Coordinate,
    to: Coordinate,
    path: Vec<Coordinate>,
    shape: Vec<Coordinate>,
}

impl LinkSpace {
    fn new<T>(nodes: &T, from: Entity, to: Entity) -> GameResult<Self>
        where T: GenericReadStorage<Component=Node>
    {
        let source_pos = try_get(nodes, from)?.at;
        let sink_pos = try_get(nodes, to)?.at;
        Ok(LinkSpace::new_pos(source_pos, sink_pos))
    }

    fn new_pos(from: Coordinate, to: Coordinate) -> Self {
        let mut path = vec![];
        let mut shape = vec![];
        let mut shape_excl = HashSet::<Coordinate>::new();
        from.for_each_in_range(NODE_RADIUS, |c| { shape_excl.insert(c); });
        to.for_each_in_range(NODE_RADIUS, |c| { shape_excl.insert(c); });
        from.for_each_in_line_to(to, |c| {
            if !shape_excl.contains(&c) { shape.push(c); }
            if c != from && c != to { path.push(c); }
        });

        LinkSpace { from, to, path, shape }
    }
}

pub fn space_for_link(map: &geom::Map, from: Coordinate, to: Coordinate) -> bool {
    let ls = LinkSpace::new_pos(from, to);
    for coord in ls.shape {
        if map.get(coord).is_some() { return false }
    }
    true
}

pub fn make_link(world: &mut World, from: Entity, to: Entity) -> GameResult<Entity> {
    make_link_parts(
        &world.entities(),
        &mut *world.write_resource(),
        &mut *world.write_resource(),
        &mut world.write_storage(),
        &mut world.write_storage(),
        &mut world.write_storage(),
        &mut world.write_storage(),
        &mut world.read_storage(),
        from, to,
    )
}

pub fn make_link_parts<T>(
    entities: &Entities,
    map: &mut geom::Map,
    areas: &geom::AreaMap,
    spaces: &mut WriteStorage<geom::Space>,
    shapes: &mut WriteStorage<draw::Shape>,
    sources: &mut WriteStorage<resource::Source>,
    links: &mut WriteStorage<Link>,
    nodes: &T,
    from: Entity, to: Entity,
) -> GameResult<Entity>
    where T: GenericReadStorage<Component=Node>
{
    let ls = LinkSpace::new(nodes, from, to)?;
    let ent = entities.create();
    shapes.insert(ent,
        draw::Shape {
            coords: ls.shape.clone(),
            color: graphics::Color::new(0.0, 0.8, 0.0, 1.0),
        }
    ).unwrap();
    let link = Link { from, to, path: ls.path };
    links.insert(ent, link.clone()).unwrap();
    map.set(spaces, ent, geom::Space::new(ls.shape))?;
    let sources_from: HashSet<Entity> = areas.find(ls.from).collect();
    let sources_to: HashSet<Entity> = areas.find(ls.to).collect();
    for &e in sources_from.intersection(&sources_to) {
        if let Some(source) = sources.get_mut(e) {
            source.add_link(&link, ent);
        }
    }
    Ok(ent)
}
