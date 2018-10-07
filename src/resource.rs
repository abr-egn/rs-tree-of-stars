use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use ggez::{
    GameResult, GameError,
};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use geom;
use graph;
use util::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Resource {
    H2,
    O2,
    H2O,
}

impl Resource {
    pub fn all() -> impl Iterator<Item=Resource> {
        const ALL: [Resource; 3] = [
            Resource::H2,
            Resource::O2,
            Resource::H2O,
        ];
        ALL.iter().cloned()
    }
}

// Epiphany: `Source` and `Sink` are *just* the input/output buffers.
// Sinks pull from available Sources until (has + incoming) >= need.
// Other behavior - production, reactor, etc. - are just inc/decs on
// the Source/Sink numbers.

#[derive(Debug, Clone)]
pub struct Pool(HashMap<Resource, usize>);

impl Pool {
    pub fn new() -> Self { Pool(HashMap::new()) }
    pub fn from<T>(t: T) -> Self
        where T: IntoIterator<Item=(Resource, usize)>
    { Pool(t.into_iter().collect()) }
    pub fn get(&self, res: Resource) -> usize { *self.0.get(&res).unwrap_or(&0) }
    pub fn set(&mut self, res: Resource, count: usize) -> usize {
        self.0.insert(res, count).unwrap_or(0)
    }
    pub fn inc(&mut self, res: Resource) { self.inc_by(res, 1) }
    pub fn inc_by(&mut self, res: Resource, count: usize) {
        *self.0.entry(res).or_insert(0) += count
    }
    pub fn dec(&mut self, res: Resource) -> GameResult<()> {
        self.dec_by(res, 1)
    }
    pub fn dec_by(&mut self, res: Resource, count: usize) -> GameResult<()> {
        match self.0.get_mut(&res) {
            Some(c) => {
                if *c >= count  {
                    *c -= count;
                    return Ok(())
                }
            },
            _ => (),
        }
        Err(GameError::UnknownError("invalid pool decrement".into()))
    }
    pub fn iter<'a>(&'a self) -> impl Iterator<Item=(Resource, usize)> + 'a {
        self.0.iter().map(|(&r, &c)| (r, c))
    }
}

#[derive(Debug)]
pub struct Source {
    pub has: Pool,
}

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

impl Source {
    pub fn new() -> Self { Source { has: Pool::new() } }
}

#[derive(Debug)]
pub struct Sink {
    pub want: Pool,
    pub has: Pool,
    pub in_transit: Pool,
    pub range: i32,
    last_pull: HashMap<Entity /* Source */, Instant>,
}

impl Component for Sink {
    type Storage = BTreeStorage<Self>;
}

impl Sink {
    pub fn new(range: i32) -> Self {
        Sink {
            want: Pool::new(), has: Pool::new(), in_transit: Pool::new(),
            range, last_pull: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Packet {
    pub sink: Entity,
    pub resource: Resource,
}

impl Component for Packet {
    type Storage = BTreeStorage<Self>;
}

const PACKET_SPEED: f32 = 2.0;
const PULL_COOLDOWN: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct Pull;

#[derive(SystemData)]
pub struct PullData<'a> {
    entities: Entities<'a>,
    now: ReadExpect<'a, super::Now>,
    graph: ReadExpect<'a, graph::Graph>,
    map: ReadExpect<'a, geom::Map>,
    nodes: ReadStorage<'a, graph::Node>,
    links: ReadStorage<'a, graph::Link>,
    motions: WriteStorage<'a, geom::Motion>,
    routes: WriteStorage<'a, graph::Route>,
    sources: WriteStorage<'a, Source>,
    sinks: WriteStorage<'a, Sink>,
    packets: WriteStorage<'a, Packet>,
}

#[derive(PartialEq, Eq)]
struct Candidate {
    source: Entity,
    route: Vec<Entity>,
    route_time: Duration,
    on_cooldown: bool,
}

impl Ord for Candidate {
    fn cmp(&self, other: &Candidate) -> Ordering { self.route_time.cmp(&other.route_time) }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Candidate) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl<'a> System<'a> for Pull {
    type SystemData = PullData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        for (entity, sink_node, sink) in (&*data.entities, &data.nodes, &mut data.sinks).join() {
            let mut need = HashMap::new();
            for (res, want) in sink.want.iter() {
                let pending = sink.has.get(res) + sink.in_transit.get(res);
                if pending < want { need.insert(res, want - pending); }
            }
            if need.is_empty() { continue }

            // Find the closest source with anything the sink needs.
            let mut candidates: Vec<Candidate> = vec![];
            let now = data.now.0;
            let sources: HashSet<Entity> = {
                // struct field sub-borrow so the filter_map closure doesn't try to borrow the
                // whole `data` struct
                let data_sources = &data.sources;
                data.map.in_range(sink_node.at(), sink.range)
                    .into_iter()
                    .filter_map(|source_ent| {
                        match data_sources.get(source_ent) {
                            Some(source) => {
                                if need.keys().any(|&res| source.has.get(res) > 0) {
                                    Some(source_ent)
                                } else {
                                    None
                                }
                            },
                            _ => None,
                        }
                    })
                    .collect()
            };
            for source_ent in sources {
                let (len, route) = if let Some(p) = data.graph.route(
                    &data.links, &data.nodes, source_ent, entity,
                ) { p } else { continue };
                let mut route_time = f32_duration(PACKET_SPEED * (len as f32));
                let mut on_cooldown = false;
                match sink.last_pull.get(&source_ent) {
                    None => (),
                    Some(&last_pull) => {
                        let since_pull = now - last_pull;
                        if since_pull < PULL_COOLDOWN {
                            let cd = PULL_COOLDOWN - since_pull;
                            route_time += cd;
                            on_cooldown = true;
                        }
                    }
                };
                candidates.push(Candidate { source: source_ent, route, route_time, on_cooldown });
            }

            if candidates.is_empty() {
                // TODO: flag for "blocked" display
                continue
            }
            candidates.sort_unstable();
            let candidate = &candidates[0];
            if candidate.on_cooldown { continue }

            let source = try_get_mut(&mut data.sources, candidate.source).unwrap();
            let coord = try_get(&data.nodes, candidate.source).unwrap().at();

            // Take the thing the sink needs the most of            
            let mut can_pull: Vec<(Resource, usize)> = vec![];
            for (res, need_amount) in need {
                if source.has.get(res) > 0 {
                    can_pull.push((res, need_amount))
                }
            }
            assert!(!can_pull.is_empty());
            can_pull.sort_unstable_by(|a, b| b.1.cmp(&a.1));
            let pull_res = can_pull[0].0;
            sink.last_pull.insert(candidate.source, now);
            source.has.dec(pull_res).unwrap();
            sink.in_transit.inc(pull_res);

            let packet = data.entities.create();
            data.packets.insert(packet, Packet {
                sink: entity,
                resource: pull_res,
            }).unwrap();
            graph::Traverse::start(
                packet,
                coord,
                &candidate.route,
                PACKET_SPEED,
                &data.graph,
                &data.links,
                &mut data.motions,
                &mut data.routes,
            ).unwrap();
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
        WriteStorage<'a, Sink>,
    );

    fn run(&mut self, (entities, route_done, packets, mut sinks): Self::SystemData) {
        for (entity, _, packet) in (&*entities, &route_done, &packets).join() {
            let sink = try_get_mut(&mut sinks, packet.sink).unwrap();
            sink.in_transit.dec(packet.resource).unwrap();
            sink.has.inc(packet.resource);
            entities.delete(entity).unwrap();
        }
    }
}

#[derive(Debug)]
pub struct Reactor {
    input: Pool,
    delay: Duration,
    output: Pool,
    in_progress: Option<Duration>,
}

impl Reactor {
    pub fn new(input: Pool, delay: Duration, output: Pool) -> Self {
        Reactor { input, delay, output, in_progress: None }
    }
}

impl Component for Reactor {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Reaction;

impl<'a> System<'a> for Reaction {
    type SystemData = (
        WriteStorage<'a, Reactor>,
        WriteStorage<'a, Source>,
        WriteStorage<'a, Sink>,
    );

    fn run(&mut self, (mut reactors, mut sources, mut sinks): Self::SystemData) {
        for (reactor, source, sink) in (&mut reactors, &mut sources, &mut sinks).join() {
            // Ensure sink pull of reactor need.
            for (res, count) in reactor.input.iter() {
                if sink.want.get(res) < count {
                    sink.want.set(res, count);
                }
            }

            // Check in progress production.
            let produce = if let Some(dur) = reactor.in_progress.as_mut() {
                *dur += super::UPDATE_DURATION;
                *dur >= reactor.delay
            } else { false };
            if produce {
                reactor.in_progress = None;
                for (res, count) in reactor.output.iter() {
                    source.has.inc_by(res, count)
                }
            }
            // If nothing's in progress (or has just finished), start.
            if reactor.in_progress.is_some() { continue }
            let has_input = reactor.input.iter().all(|(r, c)| sink.has.get(r) >= c);
            if !has_input { continue }
            for (res, count) in reactor.input.iter() {
                if count == 0 { continue }
                sink.has.dec_by(res, count).unwrap();
            }
            reactor.in_progress = Some(Duration::new(0, 0));
        }
    }
}