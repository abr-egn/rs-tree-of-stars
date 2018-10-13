use std::{
    cmp::max,
    collections::{HashSet, HashMap},
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
use error::Result;
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
    pub fn new() -> Self {
        Graph {
            data: GraphMap::new(),
            route_cache: HashMap::new(),
        }
    }
    pub fn add_link(&mut self, link: &Link, entity: Entity) {
        self.data.add_edge(link.from, link.to, entity);
        self.route_cache.clear();
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
    let from_coord = nodes.get(from).unwrap().at;
    let (len, nodes) = if let Some(p) = petgraph::algo::astar(
        /* graph= */ data,
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
        let link_ent = *data.edge_weight(nodes[ix], nodes[ix+1]).unwrap();
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

#[derive(Debug)]
pub struct AreaGraph {
    graph: Graph,
    range: i32,
}

impl AreaGraph {
    pub fn add(world: &mut World, entity: Entity, range: i32) -> Result<()> {
        let at = try_get(&world.read_storage::<Node>(), entity)?.at();
        let mut ag = AreaGraph { graph: Graph::new(), range };
        let links = world.read_storage::<Link>();
        for found in world.read_resource::<geom::Map>().in_range(at, range) {
            if let Some(link) = links.get(found) {
                ag.graph.add_link(link, found);
            }
        }
        world.write_storage().insert(entity, ag).unwrap();
        world.write_resource::<geom::AreaMap>().insert(at, range, entity);
        Ok(())
    }

    #[allow(unused)]
    pub fn graph(&self) -> &Graph { &self.graph }
    #[allow(unused)]
    pub fn range(&self) -> i32 { self.range }
    pub fn nodes_route<'a>(&'a mut self) -> (impl Iterator<Item=Entity> + 'a, Router<'a>) {
        self.graph.nodes_route()
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

#[derive(Fail, Debug)]
#[fail(display = "Path ix past the end.")]
pub struct PathIxOverflow;

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
        return Err(PathIxOverflow)?
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
    ) -> Result<()> {
        let (first_coord, p) = path_ix(route[0], 0, &world.read_storage::<Link>())?;
        let follow = FollowRoute::new(route, speed, RoutePhase::ToLink(first_coord, p));
        world.write_storage::<geom::Motion>().insert(entity,
            geom::Motion::new(start, first_coord, follow.speed))?;
        world.write_storage::<FollowRoute>().insert(entity, follow)?;
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
                let (link_ent, path_dir) = route.route[route.link_ix];
                let link = data.links.get(link_ent).unwrap();
                let node_ent = match path_dir {
                    PathDir::Fwd => link.to,
                    PathDir::Rev => link.from,
                };
                let coord = data.nodes.get(node_ent).unwrap().at;
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

pub fn make_node(world: &mut World, center: Coordinate) -> Result<Entity> {
    let ent = world.create_entity()
        .with(draw::Shape {
            coords: node_shape(center),
            color: graphics::Color::new(0.8, 0.8, 0.8, 1.0),
        })
        .with(Node { at: center })
        .build();
    world.write_resource::<geom::Map>().set(
        &mut world.write_storage::<geom::Space>(), ent,
        geom::Space::new(node_space(center)),
    )?;
    Ok(ent)
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

pub fn make_link(world: &mut World, from: Entity, to: Entity) -> Result<Entity> {
    let ls = LinkSpace::new(&world.read_storage::<Node>(), from, to)?;
    let link = Link { from, to, path: ls.path };
    let ent = world.create_entity()
        .with(draw::Shape {
            coords: ls.shape.clone(),
            color: graphics::Color::new(0.0, 0.8, 0.0, 1.0),
        })
        .with(link.clone())
        .build();
    world.write_resource::<geom::Map>().set(
        &mut world.write_storage::<geom::Space>(), ent, geom::Space::new(ls.shape))?;
    let areas = world.read_resource::<geom::AreaMap>();
    let found_from: HashSet<Entity> = areas.find(ls.from).collect();
    let found_to: HashSet<Entity> = areas.find(ls.to).collect();
    let mut ags = world.write_storage::<AreaGraph>();
    for &e in found_from.intersection(&found_to) {
        if let Some(ag) = ags.get_mut(e) {
            ag.graph.add_link(&link, ent);
        }
    }
    Ok(ent)
}
