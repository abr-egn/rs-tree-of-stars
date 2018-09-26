use std::collections::{
    hash_map,
    HashMap, HashSet,
};

use ggez::{
    nalgebra,
    GameResult, GameError,
};
use hex2d::{Coordinate, Direction, Spin};
use specs::{
    prelude::*,
    storage::BTreeStorage,
    Component,
};

type Point = nalgebra::Point2<f32>;

#[derive(Debug)]
pub struct Shape(pub Vec<Coordinate>);

impl Component for Shape {
    type Storage = VecStorage<Self>;
}

#[derive(Debug)]
pub struct Center(pub Coordinate);

impl Component for Center {
    type Storage = BTreeStorage<Self>;
}

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

#[derive(Debug)]
pub struct Link {
    pub source: Entity,
    pub sink: Entity,
    pub path: Vec<Coordinate>,  // source -> sink
}

impl Component for Link {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Motion {
    pub from: Point,
    pub to: Point,
    pub inc: f32,
    pub at: f32,
}

impl Motion {
    pub fn new(from: Coordinate, to: Coordinate, speed: f32) -> Self {
        let (fx, fy) = from.to_pixel(super::SPACING);
        let (tx, ty) = to.to_pixel(super::SPACING);
        let from = Point::new(fx, fy);
        let to = Point::new(tx, ty);
        let dist = nalgebra::distance(&from, &to);
        /* Hex center to hex center is 2 * altitude of equilateral triangle */
        let speed_scale = 3.0f32.sqrt() * super::HEX_SIDE;
        let inc = (speed * speed_scale * super::UPDATE_DELTA) / dist;
        Motion { from, to, inc, at: 0.0 }
    }
}

impl Component for Motion {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Default)]
pub struct Arrived;

impl Component for Arrived {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, Motion>,
        WriteStorage<'a, Arrived>,
    );

    fn run(&mut self, (entities, mut motions, mut arrived): Self::SystemData) {
        let mut v = Vec::new();
        for (entity, motion, ()) in (&*entities, &mut motions, !&arrived).join() {
            if motion.at >= 1.0 { continue };
            motion.at += motion.inc;
            if motion.at >= 1.0 {
                v.push(entity);
            }
        }
        for entity in v {
            arrived.insert(entity, Arrived).unwrap();
        }
    }
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

/*
#[derive(Debug)]
pub struct Packet {
    route: Vec<Entity /* Link */>,
    speed: f32,
    route_index: usize,
    path_index: usize,
    to_next: f32,
    from_hex: Coordinate,
    to_hex: Coordinate,
}

impl Packet {
    pub fn new<'a>(
        sources: &ReadStorage<'a, Source>,
        centers: &ReadStorage<'a, Center>,
        links: &ReadStorage<'a, Link>,
        source: Entity, sink: Entity, speed: f32
        ) -> GameResult<Self> {
        let source_val = game::try_get(sources, source)?;
        let route = if let Some(r) = source_val.sinks.get(&sink) { r } else {
            return Err(GameError::UnknownError("no route to sink".into()));
        };
        /*
        Packet {
            route: route.to_owned(),
            speed: speed,
            route_index: 0,
            path_index: 0,
            to_next: 0.0,
        }
        */
        unimplemented!()
    }

    pub fn done(&self) -> bool { self.route_index >= self.route.len() }

    fn update<'a>(&mut self, links: &ReadStorage<'a, Link>) {
        if self.done() { return };
        self.to_next += self.speed * super::UPDATE_DELTA;  // TODO: speed / distance
        if self.to_next >= 1.0 {
            self.to_next -= 1.0;
            self.path_index += 1;
            let link = if let Some(l) = links.get(self.route[self.route_index]) { l } else { return };
            if self.path_index >= link.path.len() {
                self.path_index = 0;
                self.route_index += 1;
            }
        }
    }
}

impl Component for Packet {
    type Storage = BTreeStorage<Self>;
}

pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        ReadStorage<'a, Link>,
        WriteStorage<'a, Packet>,
    );

    fn run(&mut self, (links, mut packets): Self::SystemData) {
        for packet in (&mut packets).join() {
            packet.update(&links);
        }
    }
}
*/

const NODE_RADIUS: i32 = 1;

pub fn make_node(world: &mut World, center: Coordinate) -> Entity {
    world.create_entity()
        .with(Center(center))
        .with(Shape(center.ring(NODE_RADIUS, Spin::CW(Direction::XY))))
        .build()
}

pub fn try_get<'a, 'b, T: Component>(storage: &'b ReadStorage<'a, T>, ent: Entity) -> GameResult<&'b T> {
    match storage.get(ent) {
        Some(t) => Ok(t),
        None => Err(GameError::UnknownError("no such component".into())),
    }
}

pub fn try_get_mut<'a, 'b, T: Component>(storage: &'b mut WriteStorage<'a, T>, ent: Entity) -> GameResult<&'b mut T> {
    match storage.get_mut(ent) {
        Some(t) => Ok(t),
        None => Err(GameError::UnknownError("no such component".into())),
    }
}

pub fn make_link(world: &mut World, source: Entity, sink: Entity) -> GameResult<Entity> {
    let mut path = vec![];
    let mut link_excl;
    {
        let centers = world.read_storage::<Center>();
        let &Center(ref source_pos) = try_get(&centers, source)?;
        let &Center(ref sink_pos) = try_get(&centers, sink)?;
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
        (hash_map::Entry::Vacant(source_route), false) => {
            source_route.insert(route.iter().cloned().collect());
            sink_sources.insert(source);
        }
        _ => return Err(GameError::UnknownError("link already exists".into())),
    };

    Ok(())
}