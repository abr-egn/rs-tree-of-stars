use std::{
    collections::HashMap,
    mem::swap,
    time::{Duration, Instant},
};

use ggez::{
    GameResult, GameError,
};
use specs::{
    prelude::*,
    storage::{BTreeStorage,GenericReadStorage},
};

use geom;
use graph::{self, Graph};
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
    range: i32,
    last_send: HashMap<Entity /* Sink */, Instant>,
    graph: Graph,  // TODO: move this to an AreaGraph component
}

impl Source {
    pub fn add_link(&mut self, link: &graph::Link, entity: Entity) {
        self.graph.add_link(link, entity)
    }
}

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

pub fn add_source<ReadNodes, ReadLinks>(
    map: &geom::Map,
    areas: &mut geom::AreaMap,
    nodes: &ReadNodes,
    links: &ReadLinks,
    sources: &mut WriteStorage<Source>,
    entity: Entity,
    has: Pool, range: i32,
)
    where ReadNodes: GenericReadStorage<Component=graph::Node>,
          ReadLinks: GenericReadStorage<Component=graph::Link>,
{
    let mut source = Source { has, range, last_send: HashMap::new(), graph: Graph::new() };
    let at = nodes.get(entity).unwrap().at();
    for found in map.in_range(at, range) {
        if let Some(link) = links.get(found) {
            source.add_link(link, found);
        }
    }
    sources.insert(entity, source).unwrap();
    areas.insert(at, range, entity);
}

#[derive(Debug)]
pub struct Sink {
    pub want: Pool,
    pub has: Pool,
    pub in_transit: Pool,
}

impl Component for Sink {
    type Storage = BTreeStorage<Self>;
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
    pub sink: Entity,
    pub resource: Resource,
}

impl Component for Packet {
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
    map: ReadExpect<'a, geom::Map>,
    nodes: ReadStorage<'a, graph::Node>,
    links: ReadStorage<'a, graph::Link>,
    motions: WriteStorage<'a, geom::Motion>,
    routes: WriteStorage<'a, graph::FollowRoute>,
    sources: WriteStorage<'a, Source>,
    sinks: WriteStorage<'a, Sink>,
    packets: WriteStorage<'a, Packet>,
}

struct Candidate {
    source: Entity,
    route: graph::Route,
    route_time: Duration,
    on_cooldown: bool,
}

impl<'a> System<'a> for Pull {
    type SystemData = PullData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        let mut sink_candidates: HashMap<Entity /* Sink */, Vec<Candidate>> = HashMap::new();
        for (source_ent, node, source) in (&*data.entities, &data.nodes, &data.sources).join() {
            // TODO: push nodes into local graph as well
            let found = data.map.in_range(node.at(), source.range);
            let mut candidates: Vec<(Entity, Candidate)> = vec![];
            for sink_ent in found {
                if sink_ent == source_ent { continue }
                let sink = if let Some(s) = data.sinks.get(sink_ent) { s } else { continue };
                let mut want = false;
                for (res, have) in source.has.iter() {
                    if have == 0 { continue }
                    if sink.want.get(res) > (sink.has.get(res) + sink.in_transit.get(res)) {
                        want = true;
                        break
                    }
                }
                if !want { continue }
                let (len, route) = if let Some(p) = source.graph.route(&data.links, &data.nodes, source_ent, sink_ent) { p } else { continue };
                let mut route_time = f32_duration(PACKET_SPEED * (len as f32));
                let on_cooldown = match source.last_send.get(&sink_ent) {
                    None => false,
                    Some(&t) => {
                        let since_send = data.now.0 - t;
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
            if candidates.is_empty() { continue }
            candidates.sort_unstable_by_key(|(_, c)| c.route_time);
            let mut tmp = (source_ent, Candidate {
                source: source_ent,
                route: vec![],
                route_time: Duration::from_millis(0),
                on_cooldown: false,
            });
            swap(&mut tmp, &mut candidates[0]);
            sink_candidates.entry(tmp.0)
                .or_insert_with(|| vec![])
                .push(tmp.1);
        }
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
            source.has.dec(pull_res).unwrap();
            sink.in_transit.inc(pull_res);

            let packet = data.entities.create();
            data.packets.insert(packet, Packet {
                sink: sink_ent,
                resource: pull_res,
            }).unwrap();
            let source_coord = data.nodes.get(candidate.source).unwrap().at();
            graph::Traverse::start(
                packet,
                source_coord,
                candidate.route.clone(),
                PACKET_SPEED,
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
            /*
        (&*entities, &route_done, &packets).par_join().for_each_with(sinks,
        |sinks, (entity, _, packet)| {
            */
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
    #[allow(unused)]
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