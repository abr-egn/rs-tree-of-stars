use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Duration,
};

use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use error::{Error, Result, or_die};
use graph;
use resource::{
    self,
    Pool, Reactor, Resource,
};
use util;

#[derive(Debug, Default)]
pub struct Pending;

impl Component for Pending {
    type Storage = NullStorage<Self>;
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    // Structure
    Strut,
    // Reactors
    CarbonSource,
    #[allow(unused)]
    Electrolysis,
    /*
    // Power
    PowerSource,
    Pylon,
    // Other
    Storage,
    */
}

#[derive(Debug, Clone)]
pub struct Packet {
    kind: Kind,
    target: Entity,
}

impl Component for Packet {
    type Storage = BTreeStorage<Self>;
}

const REACTION_TIME: Duration = Duration::from_millis(5000);
const REACTOR_RANGE: i32 = 20;
const PACKET_SPEED: f32 = 2.0;

impl Kind {
    fn make(&self, world: &mut World, entity: Entity) {
        use self::Kind::*;
        match self {
            Strut => (),
            CarbonSource => Reactor::add(
                world, entity,
                /* input=  */ Pool::from(vec![]),
                /* delay=  */ REACTION_TIME,
                /* output= */ Pool::from(vec![(Resource::C, 1)]),
                /* power=  */ 0.0,  // kJ/mol
                /* range=  */ REACTOR_RANGE,
            ),
            Electrolysis => Reactor::add(
                world, entity,
                /* input=  */ Pool::from(vec![(Resource::H2O, 2)]),
                /* delay=  */ REACTION_TIME,
                /* output= */ Pool::from(vec![(Resource::O2, 1), (Resource::H2, 2)]),
                /* power=  */ -3242.0,  // kJ/mol
                /* range=  */ REACTOR_RANGE,
            ),
        }
    }
    fn cost(&self) -> (Pool, Duration) {
        use self::Kind::*;
        match self {
            Strut => (
                Pool::from(vec![(Resource::C, 2)]),
                Duration::from_millis(10000),
            ),
            CarbonSource => (
                Pool::from(vec![(Resource::C, 2)]),
                Duration::from_millis(10000),
            ),
            Electrolysis => (
                Pool::from(vec![(Resource::C, 2)]),
                Duration::from_millis(10000),
            ),
        }
    }
    pub fn start(&self, world: &mut World, start: Entity, fork: Entity, location: Coordinate) {
        let node = graph::make_node(world, location);
        or_die(|| {
            world.write_storage().insert(node, Pending)?;
            graph::make_link(world, fork, node);
            let packet = world.create_entity()
                .with(Packet { kind: *self, target: node })
                .build();
            let route = {
                let mut areas = world.write_storage();
                let ag: &mut graph::AreaGraph = util::try_get_mut(&mut areas, start)?;
                let (_, mut router) = ag.nodes_route();
                let (_, route) = router.route(
                    &world.read_storage(), &world.read_storage(),
                    start, node,
                ).ok_or(Error::NoPath)?;
                route
            };
            let start_coord = util::try_get(&world.read_storage::<graph::Node>(), start)?.at();
            graph::Traverse::start(world, packet, start_coord, route, PACKET_SPEED);
            Ok(())
        });
    }
}

#[derive(Debug)]
pub struct Build;

impl<'a> System<'a> for Build {
    type SystemData = (
        Read<'a, LazyUpdate>,
        Entities<'a>,
        ReadStorage<'a, graph::RouteDone>,
        ReadStorage<'a, Packet>,
        WriteStorage<'a, Pending>,
    );

    fn run(&mut self, (lazy, entities, route_done, packets, mut pending): Self::SystemData) {
        for (entity, _, packet) in (&*entities, &route_done, &packets).join() {
            pending.remove(packet.target);
            entities.delete(entity).unwrap();
            let packet = packet.clone();
            lazy.exec_mut(move |world| {
                packet.kind.make(world, packet.target);
            });
        }
    }
}

#[derive(Debug)]
pub struct Factory {
    can_build: HashSet<Kind>,
    built: HashMap<Kind, usize>,
    active: Option<(Kind, Duration)>,
    queue: VecDeque<Kind>,
}

impl Factory {
    pub fn new<T: IntoIterator<Item=Kind>>(can_build: T) -> Self {
        Factory {
            can_build: can_build.into_iter().collect(),
            built: HashMap::new(),
            active: None,
            queue: VecDeque::new(),
        }
    }
    pub fn can_build(&self) -> &HashSet<Kind> { &self.can_build }
    pub fn built(&self, kind: Kind) -> usize { *self.built.get(&kind).unwrap_or(&0) }
    pub fn dec_built(&mut self, kind: Kind) -> Result<()> {
        let has = self.built(kind);
        if has == 0 {
            return Err(Error::PoolUnderflow)
        }
        self.built.insert(kind, has-1);
        Ok(())
    }
    pub fn progress(&self) -> Option<(Kind, f32)> {
        let (kind, prog) = if let Some(p) = &self.active { p } else { return None };
        let (_, delay) = kind.cost();
        Some((*kind, util::duration_f32(*prog) / util::duration_f32(delay)))
    }
    pub fn queue(&self) -> &VecDeque<Kind> { &self.queue }
    pub fn queue_push(&mut self, kind: Kind) { self.queue.push_back(kind) }
}

impl Component for Factory {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Production;

impl<'a> System<'a> for Production {
    type SystemData = (
        WriteStorage<'a, Factory>,
        WriteStorage<'a, resource::Sink>,
    );

    fn run(&mut self, (mut factories, mut sinks): Self::SystemData) {
        for (factory, sink) in (&mut factories, &mut sinks).join() {
            // Check production state
            let produced = if let Some((kind, time)) = &mut factory.active {
                *time += super::UPDATE_DURATION;
                let (_, dur) = kind.cost();
                if *time >= dur {
                    Some(*kind)
                } else { None }
            } else { None };
            // Track if something finished
            if let Some(kind) = produced {
                let count = factory.built.entry(kind).or_insert(0);
                *count += 1;
                factory.active = None;
            }
            // Request the resources for the next queued item
            let next = if let Some(f) = factory.queue.pop_front() { f } else { continue };
            let (cost, _) = next.cost();
            let mut has_all = true;
            for (res, count) in cost.iter() {
                if sink.want.get(res) != count { sink.want.set(res, count); }
                if sink.has.get(res) < count { has_all = false }
            }
            if !has_all || factory.active.is_some() {
                factory.queue.push_front(next);
                continue;
            }
            // Clear sink requests and start production.
            for (res, count) in cost.iter() {
                sink.want.set(res, 0);
                sink.has.dec_by(res, count).unwrap();
            }
            factory.active = Some((next, Duration::new(0, 0)));
        }
    }
}