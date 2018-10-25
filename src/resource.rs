use std::{
    collections::{HashMap},
    mem::swap,
    sync::mpsc::{channel, Sender},
    time::{Duration, Instant},
};

use hex2d;
use rand::{self, Rng};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use error::{
    Result,
    into_error,
    or_die,
};
use geom;
use graph;
use util::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Resource {
    H2 = 0usize,
    O2,
    H2O,
    C,
    CO2,
    CH4,
}

impl Resource {
    pub fn all() -> impl Iterator<Item=Resource> {
        const ALL: [Resource; 6] = [
            Resource::H2,
            Resource::O2,
            Resource::H2O,
            Resource::C,
            Resource::CO2,
            Resource::CH4,
        ];
        ALL.iter().cloned()
    }
}

// Epiphany: `Source` and `Sink` are *just* the input/output buffers.
// Sinks pull from available Sources until (has + incoming) >= need.
// Other behavior - production, reactor, etc. - are just inc/decs on
// the Source/Sink numbers.

#[derive(Debug, Clone)]
pub struct Pool {
    count: [usize; 6],
    cap: [usize; 6],
}

#[derive(Fail, Debug)]
#[fail(display = "Pool underflow.")]
pub struct PoolUnderflow;

impl Pool {
    pub fn new() -> Self {
        Pool {
            count: [0, 0, 0, 0, 0, 0],
            cap: [6, 6, 6, 6, 6, 6],
        }
    }
    pub fn from<T>(t: T) -> Self
        where T: IntoIterator<Item=(Resource, usize)>
    {
        let mut p = Pool::new();
        for (res, count) in t.into_iter() {
            p.set(res, count);
        }
        p
    }
    #[allow(unused)]
    pub fn from_cap<R, C>(r: R, c: C) -> Self
        where R: IntoIterator<Item=(Resource, usize)>,
              C: IntoIterator<Item=(Resource, usize)>,
    {
        let mut p = Pool::new();
        for (res, count) in c.into_iter() {
            p.set_cap(res, count)
        }
        for (res, count) in r.into_iter() {
            p.set(res, count);
        }
        p
    }
    pub fn get(&self, res: Resource) -> usize { self.count[res as usize] }
    pub fn cap(&self, res: Resource) -> usize { self.cap[res as usize] }
    fn do_cap(&self, res: Resource, count: usize) -> (usize, Option<usize>) {
        let c = self.cap[res as usize];
        if c < count {
            (c, Some(count - c))
        } else { (count, None) }
    }
    pub fn set(&mut self, res: Resource, count: usize) -> Option<usize> {
        let (c, o) = self.do_cap(res, count);
        self.count[res as usize] = c;
        o
    }
    pub fn inc(&mut self, res: Resource) -> Option<usize> { self.inc_by(res, 1) }
    pub fn inc_by(&mut self, res: Resource, count: usize) -> Option<usize> {
        let new = self.get(res) + count;
        self.set(res, new)
    }
    pub fn dec(&mut self, res: Resource) -> Result<()> {
        self.dec_by(res, 1)
    }
    pub fn dec_by(&mut self, res: Resource, count: usize) -> Result<()> {
        if self.count[res as usize] >= count {
            self.count[res as usize] -= count;
            return Ok(())
        }
        Err(PoolUnderflow.into())
    }
    #[allow(unused)]
    pub fn set_cap(&mut self, res: Resource, cap: usize) {
        self.cap[res as usize] = cap;
    }
    pub fn iter<'a>(&'a self) -> impl Iterator<Item=(Resource, usize)> + 'a {
        self.count.iter().enumerate().map(|(u, &c)| (unsafe { ::std::mem::transmute::<usize, Resource>(u) }, c))
    }
}

#[derive(Debug)]
pub struct Source {
    pub has: Pool,
    last_send: HashMap<Entity /* Sink */, Instant>,
}

impl Source {
    pub fn add(world: &mut World, entity: Entity, has: Pool, range: i32) {
        or_die(|| {
            graph::AreaGraph::add(world, entity, range)?;
            world.write_storage().insert(entity, Source { has, last_send: HashMap::new() })?;
            Ok(())
        });
    }
}

impl Component for Source {
    type Storage = DenseVecStorage<Self>;
}

#[derive(Debug)]
pub struct Sink {
    pub want: Pool,
    pub has: Pool,
    pub in_transit: Pool,
}

impl Component for Sink {
    type Storage = DenseVecStorage<Self>;
}

impl Sink {
    pub fn new() -> Self {
        Sink {
            want: Pool::new(), has: Pool::new(), in_transit: Pool::new(),
        }
    }
}

#[derive(Debug)]
pub struct Packet {
    pub resource: Resource,
}

impl Component for Packet {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Target {
    pub node: Entity,
}

impl Component for Target {
    type Storage = BTreeStorage<Self>;
}

const PACKET_SPEED: f32 = 2.0;
const SEND_COOLDOWN: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct Pull;

#[derive(SystemData)]
pub struct PullData<'a> {
    entities: Entities<'a>,
    now: ReadExpect<'a, super::Now>,
    nodes: ReadStorage<'a, graph::Node>,
    links: ReadStorage<'a, graph::Link>,
    graphs: WriteStorage<'a, graph::AreaGraph>,
    sources: WriteStorage<'a, Source>,
    sinks: WriteStorage<'a, Sink>,
    lazy: Read<'a, LazyUpdate>,
}

#[derive(Debug)]
struct Candidate {
    source: Entity,
    route: graph::Route,
    route_time: Duration,
    on_cooldown: bool,
}

// Give what would be a closure a name so it shows up on profiles
fn pull_worker(
    sinks: &WriteStorage<Sink>,
    links: &ReadStorage<graph::Link>,
    nodes: &ReadStorage<graph::Node>,
    now: &ReadExpect<super::Now>,
    sender: &mut Sender<(Entity, Candidate)>,
    source_ent: Entity,
    source: &mut Source,
    ag: &mut graph::AreaGraph,
) {
    let mut candidates: Vec<(Entity, Candidate)> = vec![];
    let (nodes_iter, mut router) = ag.nodes_route();
    for sink_ent in nodes_iter {
        let sink = if let Some(s) = sinks.get(sink_ent) { s } else { continue };
        let mut want = false;
        for (res, have) in source.has.iter() {
            if have == 0 { continue }
            if sink.want.get(res) > (sink.has.get(res) + sink.in_transit.get(res)) {
                want = true;
                break
            }
        }
        if !want { continue }
        let (len, route) = match router.route(links, nodes, source_ent, sink_ent) {
            None => continue,
            Some(p) => p,
        };
        let mut route_time = f32_duration((len as f32) / PACKET_SPEED);
        let on_cooldown = match source.last_send.get(&sink_ent) {
            None => false,
            Some(&t) => {
                let since_send = now.0 - t;
                if since_send < SEND_COOLDOWN {
                    route_time += SEND_COOLDOWN - since_send;
                    true
                } else { false }
            }
        };
        candidates.push((sink_ent, Candidate {
            source: source_ent, route, route_time, on_cooldown,
        }));
    }
    if candidates.is_empty() { return }
    candidates.sort_unstable_by_key(|(_, c)| c.route_time);
    let mut tmp = (source_ent, Candidate {
        source: source_ent,
        route: vec![],
        route_time: Duration::from_millis(13),
        on_cooldown: false,
    });
    swap(&mut tmp, &mut candidates[0]);
    or_die(|| { sender.send(tmp)?; Ok(()) });
}

impl<'a> System<'a> for Pull {
    type SystemData = PullData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        let sink_candidates = {
            let sinks = &data.sinks;
            let links = &data.links;
            let nodes = &data.nodes;
            let now = &data.now;
            let (sender, receiver) = channel::<(Entity, Candidate)>();
            (&*data.entities, &mut data.sources, &mut data.graphs).par_join().for_each_with(sender,
                |sender, (source_ent, source, ag)| {
                pull_worker(sinks, links, nodes, now, sender, source_ent, source, ag)
            });
            let mut sink_candidates = HashMap::<Entity, Vec<Candidate>>::new();
            for (sink_ent, candidate) in receiver {
                sink_candidates.entry(sink_ent)
                    .or_insert_with(|| vec![])
                    .push(candidate);
            }
            sink_candidates
        };
        for (sink_ent, mut candidates) in sink_candidates {
            if candidates.is_empty() { continue }
            candidates.sort_unstable_by_key(|c| c.route_time);
            let candidate = &candidates[0];
            if candidate.on_cooldown { continue }
            let source = if let Some(s) = data.sources.get_mut(candidate.source) { s } else { continue };
            let sink = if let Some(s) = data.sinks.get_mut(sink_ent) { s } else { continue };

            // Take the thing the sink needs the most of
            let mut can_pull: Vec<(Resource, usize)> = vec![];
            for (res, want) in sink.want.iter() {
                let pending = sink.has.get(res) + sink.in_transit.get(res);
                if pending < want && source.has.get(res) > 0 {
                    can_pull.push((res, want - pending));
                }
            }
            if can_pull.is_empty() { continue }
            can_pull.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            let pull_res = can_pull[0].0;

            source.last_send.insert(sink_ent, data.now.0);
            or_die(|| source.has.dec(pull_res));
            sink.in_transit.inc(pull_res);

            let source_coord = {
                let nodes = &data.nodes;
                or_die(|| try_get(nodes, candidate.source)).at()
            };
            let route = candidate.route.clone();
            data.lazy.exec_mut(move |world| {
                let packet = world.create_entity()
                    .with(Packet { resource: pull_res })
                    .with(Target { node: sink_ent })
                    .build();
                graph::Traverse::start(
                    world,
                    packet,
                    source_coord,
                    route,
                    PACKET_SPEED,
                );
            });
        }
    }
}

#[derive(Debug)]
pub struct Receive;

impl<'a> System<'a> for Receive {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, graph::RouteDone>,
        ReadStorage<'a, Packet>,
        ReadStorage<'a, Target>,
        WriteStorage<'a, Sink>,
    );

    fn run(&mut self, (entities, route_done, packets, targets, mut sinks): Self::SystemData) {
        or_die(|| {
        for (entity, _, packet, target) in (&*entities, &route_done, &packets, &targets).join() {
            let sink = try_get_mut(&mut sinks, target.node)?;
            sink.in_transit.dec(packet.resource)?;
            sink.has.inc(packet.resource);
            entities.delete(entity)?;
        };
        Ok(())
        })
    }
}

#[derive(Debug)]
pub struct Reactor {
    input: Pool,
    delay: Duration,
    tick_energy: f64,
    output: Pool,
    reaction: Option<Reaction>,
}

#[derive(Debug)]
struct Reaction {
    progress: Duration,
    energy: f64,
}

impl Reactor {
    pub fn progress(&self) -> Option<f32> {
        let prog = if let Some(r) = &self.reaction { r.progress } else { return None };
        Some(duration_f32(prog) / duration_f32(self.delay))
    }
}

impl Reactor {
    #[allow(unused)]
    pub fn new(input: Pool, delay: Duration, output: Pool) -> Self {
        Reactor { input, delay, tick_energy: 0.0, output, reaction: None }
    }

    fn tick(&mut self) -> bool {
        let r = if let Some(r) = self.reaction.as_mut() { r } else { return false };
        if r.energy + self.tick_energy < 0.0 { return false }
        r.energy += self.tick_energy;
        if self.tick_energy > 0.0 && r.energy > self.tick_energy {
            // TODO: indicate energy waste
            r.energy = self.tick_energy
        }
        r.progress += super::UPDATE_DURATION;
        r.progress >= self.delay
    }
}

impl Component for Reactor {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Default)]
pub struct Waste;

impl Component for Waste {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct RunReactors;

impl<'a> System<'a> for RunReactors {
    type SystemData = (
        ReadStorage<'a, graph::Node>,
        WriteStorage<'a, Reactor>,
        WriteStorage<'a, Source>,
        WriteStorage<'a, Sink>,
        Read<'a, LazyUpdate>,
    );

    fn run(&mut self, (nodes, mut reactors, mut sources, mut sinks, lazy): Self::SystemData) {
        for (node, reactor, source, sink) in (&nodes, &mut reactors, &mut sources, &mut sinks).join() {
            // Ensure sink pull of reactor need.
            for (res, count) in reactor.input.iter() {
                if sink.want.get(res) < count {
                    sink.want.set(res, count);
                }
            }

            // Check in progress production.
            let produce = reactor.tick();
            if produce {
                reactor.reaction = None;
                for (res, count) in reactor.output.iter() {
                    if let Some(waste) = source.has.inc_by(res, count) {
                        spawn_waste(&lazy, node.at(), res, waste);
                    }
                }
            }

            // If nothing's in progress (or has just finished), start.
            if reactor.reaction.is_some() { continue }
            let has_input = reactor.input.iter().all(|(r, c)| sink.has.get(r) >= c);
            if !has_input { continue }
            let needs_output = reactor.output.iter().any(|(r, c)| source.has.get(r) < c);
            if !needs_output { continue }
            for (res, count) in reactor.input.iter() {
                if count == 0 { continue }
                sink.has.dec_by(res, count).unwrap();
            }
            reactor.reaction = Some(Reaction { progress: Duration::new(0, 0), energy: 0.0 });
        }
    }
}

fn spawn_waste(lazy: &LazyUpdate, center: ::hex2d::Coordinate, res: Resource, count: usize) {
    lazy.exec_mut(move |world| {
        let mut rng = rand::thread_rng();
        let targets = center.ring(5, hex2d::Spin::CW(hex2d::Direction::XY));
        for _ in 0..count {
            let target = targets[rng.gen_range::<usize>(0, targets.len())];
            world.create_entity()
                .with(Packet { resource: res })
                .with(geom::Motion::new(center, target, PACKET_SPEED))
                .with(Waste)
                .build();
        }
    });
}

#[derive(Debug)]
pub struct ClearWaste;

impl<'a> System<'a> for ClearWaste {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Waste>,
        ReadStorage<'a, geom::MotionDone>,
    );

    fn run(&mut self, (entities, wastes, arrived): Self::SystemData) {
        or_die(|| {
            for (entity, _, _) in (&*entities, &wastes, &arrived).join() {
                entities.delete(entity)?;
            }
            Ok(())
        });
    }
}

#[derive(Debug, Default)]
pub struct Storage;

impl Component for Storage {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct DoStorage;

#[derive(Fail, Debug)]
#[fail(display = "Storage overflow.")]
pub struct StorageOverflow;

impl<'a> System<'a> for DoStorage {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Storage>,
        WriteStorage<'a, Source>,
        WriteStorage<'a, Sink>,
    );

    fn run(&mut self, (entities, stores, mut sources, mut sinks): Self::SystemData) {
        for (_entity, _, source, sink) in (&*entities, &stores, &mut sources, &mut sinks).join() {
            for res in Resource::all() {
                let has = sink.has.get(res);
                if has > 0 {
                    //println!("{:?}: storing {:?}", entity, has);
                    sink.has.set(res, 0);
                    or_die(|| {
                        source.has.inc_by(res, has)
                            .map_or(Ok(()), |_| Err(into_error(StorageOverflow)))
                    });
                }
                let pending = source.has.get(res);
                let want = if source.has.cap(res) > pending {
                    source.has.cap(res) - pending
                } else { 0 };
                if want != sink.want.get(res) {
                    //println!("{:?}: pending {:?} (has {:?}, in transit {:?}) => want {:?}", entity, pending, source.has.get(res), sink.in_transit.get(res), want);
                    sink.want.set(res, want);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Burn {
    cooldown: Duration,
    last: Option<Instant>,
}

impl Burn {
    pub fn new(cooldown: Duration) -> Self {
        Burn { cooldown, last: None }
    }
}

impl Component for Burn {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct DoBurn;

impl<'a> System<'a> for DoBurn {
    type SystemData = (
        ReadExpect<'a, super::Now>,
        WriteStorage<'a, Burn>,
        WriteStorage<'a, Sink>,
    );

    fn run(&mut self, (now, mut burns, mut sinks): Self::SystemData) {
        for (burn, sink) in (&mut burns, &mut sinks).join() {
            if let Some(t) = burn.last {
                if now.0 - t < burn.cooldown { continue }
            }
            for res in Resource::all() {
                if sink.has.get(res) > 0 {
                    or_die(|| sink.has.dec(res));
                    burn.last = Some(now.0);
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Pylon;

impl Component for Pylon {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct DistributePower;

#[derive(SystemData)]
pub struct DistributePowerData<'a> {
    reactors: WriteStorage<'a, Reactor>,
    areas: ReadStorage<'a, geom::AreaSet>,
    pylons: ReadStorage<'a, Pylon>,
}

impl<'a> System<'a> for DistributePower {
    type SystemData = DistributePowerData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        // power sink -> [(source, power, surplus)]
        let mut available: HashMap<Entity, Vec<(Entity, f64, f64)>> = HashMap::new();
        for (area, _) in (&data.areas, &data.pylons).join() {
            let mut supply = 0.0;
            let mut demand = 0.0;
            for entity in area.nodes() {
                if let Some(reactor) = data.reactors.get(entity) {
                    if let Some(reaction) = &reactor.reaction {
                        if reactor.tick_energy > 0.0 {
                            supply += reaction.energy;
                        } else {
                            demand += reaction.energy + reactor.tick_energy;
                        }
                    }
                }
            }
            let surplus = supply - demand;
            /*
            if reactor.tick_energy < 0.0 { continue }
            let energy = if let Some(r) = &reactor.reaction { r.energy } else { continue };
            */
        }
    }
}