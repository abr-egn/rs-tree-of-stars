use std::{
    cmp::max,
    collections::{
        HashSet, HashMap,
    },
};

use ggez::graphics;
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
use error::{
    Error, Result,
    or_die,
};
use geom;
use util::*;

type GraphData = GraphMap<Entity, Entity, petgraph::Undirected>;

#[derive(Debug)]
pub struct Graph {
    data: GraphData,
    route_cache: HashMap<(Entity, Entity), Option<(usize, Route)>>,
}

pub type Route = Vec<(Entity, PathDir)>;

impl Graph {
    fn new() -> Self {
        Graph {
            data: GraphMap::new(),
            route_cache: HashMap::new(),
        }
    }
    fn add_link(&mut self, link: &Link, entity: Entity) {
        self.data.add_edge(link.from, link.to, entity);
        self.route_cache.clear();
    }
    fn add_link_to(&mut self, from: Entity, to: Entity, link_ent: Entity) {
        self.data.add_edge(from, to, link_ent);
        self.route_cache.clear();
    }
    fn remove_link(&mut self, from: Entity, to: Entity) -> Option<Entity> {
        let ret = self.data.remove_edge(from, to);
        if ret.is_some() { self.route_cache.clear() }
        ret
    }
    pub fn nodes_route<'a>(&'a mut self) -> (impl Iterator<Item=Entity> + 'a, Router<'a>) {
        (self.data.nodes(), Router { data: &self.data, route_cache: &mut self.route_cache })
    }
}

pub struct Router<'a> {
    data: &'a GraphData,
    route_cache: &'a mut HashMap<(Entity, Entity), Option<(usize, Route)>>,
}

impl<'a> Router<'a> {
    pub fn route(
        &mut self, links: &ReadStorage<Link>, nodes: &ReadStorage<Node>,
        from: Entity, to: Entity,
    ) -> Option<(usize, Route)> {
        let data = self.data;
        self.route_cache.entry((from, to))
            .or_insert_with(|| calc_route(data, links, nodes, from, to))
            .clone()
    }
}

fn calc_route(
    data: &GraphData, links: &ReadStorage<Link>, nodes: &ReadStorage<Node>,
    from: Entity, to: Entity,
) -> Option<(usize, Route)> {
    or_die(|| {
        let from_coord = try_get(nodes, from)?.at;
        let (len, nodes) = if let Some(p) = petgraph::algo::astar(
            /* graph= */ data,
            /* start= */ from,
            /* is_goal= */ |ent| { ent == to },
            /* edge_cost= */ |(_, _, &link_ent)| or_die(|| {
                Ok(try_get(links, link_ent)?.path.len())
            }),
            /* estimate_cost= */ |ent| or_die(|| {
                let ent_coord = try_get(nodes, ent)?.at;
                Ok(max(0, from_coord.distance(ent_coord) - 2) as usize)
            }),
        ) { p } else { return Ok(None) };
        let mut route: Vec<(Entity, PathDir)> = vec![];
        for ix in 0..nodes.len()-1 {
            let link_ent = *data.edge_weight(nodes[ix], nodes[ix+1])
                .ok_or_else(|| Error::NoSuchEdge)?;
            let link = try_get(links, link_ent)?;
            route.push((link_ent, if link.from == nodes[ix] {
                PathDir::Fwd
            } else if link.to == nodes[ix] {
                PathDir::Rev
            } else {
                panic!("invalid link data")
            }))
        }
        Ok(Some((len, route)))
    })
}

pub type AreaGraph = geom::AreaWatch<Graph>;

impl AreaGraph {
    pub fn add(world: &mut World, parent: Entity, range: i32) -> Result<()> {
        let res = {
            let nodes = world.read_storage::<Node>();
            let entities = world.entities();
            Self::build(world, parent, range, |found| {
                let mut graph = Graph::new();
                for (entity, node, _) in (&*entities, &nodes, &found).join() {
                    for (&to, &link_ent) in &node.links {
                        if graph.data.contains_edge(entity, to) || graph.data.contains_edge(to, entity) {
                            continue
                        }
                        if found.contains(to.id()) {
                            graph.add_link_to(entity, to, link_ent);
                        }
                    }
                }
                graph
            })
        }?.insert(world);
        res
    }
    pub fn nodes_route<'a>(&'a mut self) -> (impl Iterator<Item=Entity> + 'a, Router<'a>) {
        let ex = &self.exclude;
        let (iter, router) = self.data.nodes_route();
        (iter.filter(move |e| !ex.contains(e)), router)
    }
}

impl Component for AreaGraph {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Debug, Copy, Clone)]
pub enum PathDir {
    Fwd,
    Rev,
}

#[derive(Debug)]
pub struct Node {
    at: Coordinate,
    links: HashMap</* Node */ Entity,/* Link */ Entity>,
}

impl Node {
    pub fn at(&self) -> Coordinate { self.at }
}

impl Component for Node {
    type Storage = DenseVecStorage<Self>;
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
) -> Result<(Coordinate, PathCoord)> {
    let link = try_get(links, link_ent)?;
    let coord_ix = match path_dir {
        PathDir::Fwd => ix,
        PathDir::Rev => link.path.len() - 1 - ix,
    };
    if ix >= link.path.len() {
        return Err(Error::PathIxOverflow)?
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
        world: &mut World,
        entity: Entity,
        start: Coordinate,
        route: Route,
        speed: f32,
    ) {
        or_die(|| {
            let (first_coord, p) = path_ix(route[0], 0, &world.read_storage::<Link>())?;
            let follow = FollowRoute::new(route, speed, RoutePhase::ToLink(first_coord, p));
            world.write_storage::<geom::Motion>().insert(entity,
                geom::Motion::new(start, first_coord, follow.speed))?;
            world.write_storage::<FollowRoute>().insert(entity, follow)?;
            Ok(())
        })
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
            let to_coord = {
                let links = &data.links;
                let nodes = &data.nodes;
                or_die(|| {
                    if link_next {
                        // TODO: This can fail when a link is deleted.  Detect and tag.
                        let (coord, more) = path_ix(
                            route.route[route.link_ix],
                            route.coord_ix,
                            links
                        )?;
                        route.phase = RoutePhase::ToLink(coord, more);
                        Ok(coord)
                    } else {
                        let (link_ent, path_dir) = route.route[route.link_ix];
                        let link = try_get(links, link_ent)?;
                        let node_ent = match path_dir {
                            PathDir::Fwd => link.to,
                            PathDir::Rev => link.from,
                        };
                        let coord = try_get(nodes, node_ent)?.at;
                        route.phase = RoutePhase::ToNode(coord);
                        Ok(coord)
                    }
                })
            };
            more_motion.push(entity);  // arrival flag clear
            let rem = motion.at - 1.0;
            *motion = geom::Motion::new(from_coord, to_coord, route.speed);
            motion.at = rem;
        }
        for entity in more_motion {
            data.motion_done.remove(entity);
        }
        or_die(|| {
            for entity in no_more_route {
                data.route_done.insert(entity, RouteDone)?;
            }
            Ok(())
        });
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

pub fn make_node(world: &mut World, center: Coordinate) -> Entity {
    let ent = world.create_entity()
        .with(draw::Shape {
            coords: node_shape(center),
            color: graphics::Color::new(0.8, 0.8, 0.8, 1.0),
        })
        .with(Node { at: center, links: HashMap::new() })
        .build();
    or_die(|| world.write_resource::<geom::Map>().set(
        &mut world.write_storage::<geom::Space>(), ent,
        geom::Space::new(node_space(center)),
    ));
    let mut areas = world.write_storage::<geom::AreaSet>();
    let map = world.read_resource::<geom::AreaMap>();
    for (area, _) in (&mut areas, map.find(center)).join() {
        area.data.insert(ent.clone());
    }

    ent
}

pub struct LinkRange(i32);

impl LinkRange {
    pub fn new(range: i32) -> Self { LinkRange(range) }
    pub fn get(&self) -> i32 { self.0 }
}

impl Component for LinkRange {
    type Storage = BTreeStorage<Self>;
}

struct LinkSpace {
    from: Coordinate,
    to: Coordinate,
    path: Vec<Coordinate>,
    shape: Vec<Coordinate>,
}

impl LinkSpace {
    fn new<T>(nodes: &T, from: Entity, to: Entity) -> Result<Self>
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

pub fn link_shape(from: Coordinate, to: Coordinate) -> Vec<Coordinate> {
    LinkSpace::new_pos(from, to).shape
}

pub fn can_link(world: &World, from: Entity, to: Entity) -> bool {
    let nodes = world.read_storage::<Node>();
    let from_node = if let Some(n) = nodes.get(from) { n } else { return false };
    let to_node = if let Some(n) = nodes.get(to) { n } else { return false };
    if from_node.links.contains_key(&to) || to_node.links.contains_key(&from) { return false };
    if !space_for_link(&world.read_resource(), from_node.at(), to_node.at()) { return false };

    let ranges = world.read_storage::<LinkRange>();
    let from_range = if let Some(r) = ranges.get(from) { r.get() } else { return false };
    let to_range = if let Some(r) = ranges.get(to) { r.get() } else { return false };
    if max(from_range, to_range) < from_node.at().distance(to_node.at()) { return false };

    true
}

pub fn make_link(world: &mut World, from: Entity, to: Entity) -> Entity {
    let ls = or_die(|| LinkSpace::new(&world.read_storage::<Node>(), from, to));
    let link = Link { from, to, path: ls.path };
    let ent = world.create_entity()
        .with(draw::Shape {
            coords: ls.shape.clone(),
            color: graphics::Color::new(0.0, 0.8, 0.0, 1.0),
        })
        .with(link.clone())
        .build();
    let shape = ls.shape;
    or_die(|| world.write_resource::<geom::Map>().set(
        &mut world.write_storage::<geom::Space>(), ent, geom::Space::new(shape)));
    let areas = world.read_resource::<geom::AreaMap>();
    let found_from = areas.find(ls.from);
    let found_to = areas.find(ls.to);
    let mut graphs = world.write_storage::<AreaGraph>();
    for (ag, _) in (&mut graphs, found_from & found_to).join() {
        ag.data.add_link(&link, ent);
    }
    or_die(|| {
        let mut nodes = world.write_storage::<Node>();
        try_get_mut(&mut nodes, from)?.links.insert(to, ent);
        try_get_mut(&mut nodes, to)?.links.insert(from, ent);
        Ok(())
    });
    ent
}

pub fn delete_link(world: &mut World, link_ent: Entity) {
    or_die(|| {
        world.write_resource::<geom::Map>().clear(&mut world.write_storage(), link_ent)?;
        {
            let links = world.read_storage::<Link>();
            let link: &Link = try_get(&links, link_ent)?;
            let from = try_get(&world.read_storage::<Node>(), link.from)?.at();
            let to = try_get(&world.read_storage::<Node>(), link.to)?.at();
            let areas = world.read_resource::<geom::AreaMap>();
            let found_from = areas.find(from);
            let found_to = areas.find(to);
            let mut graphs = world.write_storage::<AreaGraph>();
            for (ag, _) in (&mut graphs, found_from & found_to).join() {
                ag.data.remove_link(link.from, link.to)
                    .map_or(Err(Error::NoSuchEdge), |e| {
                        if e == link_ent { Ok(()) }
                        else { Err(Error::WrongEdge) }
                    })?;
            }
        }
        Ok(())
    })
}