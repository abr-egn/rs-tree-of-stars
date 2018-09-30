use std::{
    collections::HashMap,
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
struct Connection {
    route: Vec<Entity /* Node */>,
    last_pull: Instant,
}

#[derive(Debug)]
pub struct Sink {
    pub want: usize,
    pub count: usize,
    pub in_transit: usize,
    sources: HashMap<Entity /* Source */, Connection>,
}

impl Component for Sink {
    type Storage = BTreeStorage<Self>;
}

impl Sink {
    pub fn new(want: usize) -> Self {
        Sink {
            want, count: 0, in_transit: 0,
            sources: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Packet;

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
    centers: ReadStorage<'a, geom::Center>,
    links: ReadStorage<'a, graph::Link>,
    nodes: ReadStorage<'a, graph::Node>,
    motions: WriteStorage<'a, geom::Motion>,
    routes: WriteStorage<'a, graph::Route>,
    sources: WriteStorage<'a, Source>,
    sinks: WriteStorage<'a, Sink>,
    packets: WriteStorage<'a, Packet>,
}

impl<'a> System<'a> for Pull {
    type SystemData = PullData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        for sink in (&mut data.sinks).join() {
            if sink.count + sink.in_transit >= sink.want { continue }
            let mut candidates: Vec<(Duration, Entity, bool)> = vec![];
            let now = Instant::now();
            for (source_ent, conn) in &sink.sources {
                let source = try_get_mut(&mut data.sources, *source_ent).unwrap();
                if source.has == 0 { continue }
                let mut route_time = f32_duration(
                    PACKET_SPEED * (graph::route_len(&conn.route, &data.links, &data.nodes).unwrap() as f32));
                let mut on_cd = false;
                let since_pull = now - conn.last_pull;
                if since_pull < PULL_COOLDOWN {
                    route_time += PULL_COOLDOWN - since_pull;
                    on_cd = true;
                }
                candidates.push((route_time, *source_ent, on_cd));
            }
            candidates.sort_unstable();
            let (_, source_ent, on_cd) = candidates[0];
            if on_cd { continue }

            let conn = sink.sources.get_mut(&source_ent).unwrap();
            let source = try_get_mut(&mut data.sources, source_ent).unwrap();
            let coord = try_get(&data.centers, source_ent).unwrap().0;

            conn.last_pull = now;
            source.has -= 1;
            sink.in_transit += 1;

            let packet = data.entities.create();
            data.packets.insert(packet, Packet).unwrap();
            graph::Route::start(
                packet,
                coord,
                &conn.route,
                PACKET_SPEED,
                &data.links,
                &data.nodes,
                &mut data.motions,
                &mut data.routes,
            ).unwrap();
        }
    }
}