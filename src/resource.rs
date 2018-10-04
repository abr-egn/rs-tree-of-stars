use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use geom;
use graph;
use util::*;

// Epiphany: `Source` and `Sink` are *just* the input/output buffers.
// Sinks pull from available Sources until (has + incoming) >= need.
// Other behavior - production, reactor, etc. - are just inc/decs on
// the Source/Sink numbers.

#[derive(Debug)]
pub struct Source {
    pub has: usize,
}

impl Component for Source {
    type Storage = BTreeStorage<Self>;
}

impl Source {
    pub fn new() -> Self { Source { has: 0 } }
}

#[derive(Debug)]
pub struct Sink {
    pub want: usize,
    pub range: i32,
    pub has: usize,
    pub in_transit: usize,
    last_pull: HashMap<Entity /* Source */, Instant>,
}

impl Component for Sink {
    type Storage = BTreeStorage<Self>;
}

impl Sink {
    pub fn new(want: usize, range: i32) -> Self {
        Sink {
            want, range, has: 0, in_transit: 0,
            last_pull: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Packet {
    sink: Entity,
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
            if sink.has + sink.in_transit >= sink.want { continue }

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
                            Some(source) if source.has > 0 => Some(source_ent),
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

            sink.last_pull.insert(candidate.source, now);
            source.has -= 1;
            sink.in_transit += 1;

            let packet = data.entities.create();
            data.packets.insert(packet, Packet { sink: entity }).unwrap();
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
            sink.in_transit -= 1;
            sink.has += 1;
            entities.delete(entity).unwrap();
        }
    }
}