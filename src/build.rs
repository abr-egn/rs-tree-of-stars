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
use power::Power;
use reactor::{Progress, Reactor};
use resource::{
    self,
    Pool, Resource,
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
    fn cost(&self) -> (Pool, /*power=*/ f32, Duration) {
        use self::Kind::*;
        match self {
            Strut => (
                Pool::from(vec![(Resource::C, 2)]), -100.0,
                Duration::from_millis(10000),
            ),
            CarbonSource => (
                Pool::from(vec![(Resource::C, 2)]), -100.0,
                Duration::from_millis(10000),
            ),
            Electrolysis => (
                Pool::from(vec![(Resource::C, 2)]), -100.0,
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
    queue: VecDeque<Kind>,
}

impl Factory {
    pub fn add<T: IntoIterator<Item=Kind>>(
        world: &mut World, entity: Entity,
        can_build: T, range: i32,
    ) {
        or_die(|| {
            graph::AreaGraph::add(world, entity, range)?;
            world.write_storage().insert(entity, resource::Sink::new())?;
            world.write_storage().insert(entity, Power::new())?;
            world.write_storage().insert(entity, Progress::new())?;
            world.write_storage().insert(entity, Factory {
                can_build: can_build.into_iter().collect(),
                built: HashMap::new(),
                queue: VecDeque::new(),
            })?;
            Ok(())
        });
    }
    pub fn can_build(&self) -> &HashSet<Kind> { &self.can_build }
    pub fn built(&self, kind: Kind) -> usize { *self.built.get(&kind).unwrap_or(&0) }
    pub fn inc_built(&mut self, kind: Kind) {
         let count = self.built.entry(kind).or_insert(0);
        *count += 1;
    }
    pub fn dec_built(&mut self, kind: Kind) -> Result<()> {
        let has = self.built(kind);
        if has == 0 {
            return Err(Error::PoolUnderflow)
        }
        self.built.insert(kind, has-1);
        Ok(())
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
        WriteStorage<'a, Progress>,
        WriteStorage<'a, Power>,
    );

    fn run(&mut self, (mut factories, mut sinks, mut progs, mut powers): Self::SystemData) {
        for (factory, sink, progress, power) in (&mut factories, &mut sinks, &mut progs, &mut powers).join() {
            // Check production state
            if progress.at().map_or(false, |p| p > 1.0) {
                progress.clear();
                power.clear::<Self>();
                let kind = factory.queue.pop_front().unwrap();
                factory.inc_built(kind);
            }
            
            // Request the resources for the next queued item
            let next = if let Some(f) = factory.queue.front() { f } else { continue };
            let (cost, build_power, time) = next.cost();
            let mut has_all = true;
            for (res, count) in cost.iter() {
                if sink.want.get(res) != count { sink.want.set(res, count); }
                if sink.has.get(res) < count { has_all = false }
            }
            if !has_all || progress.at().is_some() {
                continue;
            }
            // Start requesting power, and only continue if we're getting any.
            power.set::<Self>(build_power);
            if power.ratio() == 0.0 { continue }
            // Clear sink requests and start production.
            for (res, count) in cost.iter() {
                sink.want.set(res, 0);
                sink.has.dec_by(res, count).unwrap();
            }
            progress.start(time);
        }
    }
}