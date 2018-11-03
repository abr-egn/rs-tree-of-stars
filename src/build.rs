use std::time::Duration;

use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use error::{Error, or_die};
use graph;
use resource::{
    Pool, Reactor, Resource,
};
use util;

#[derive(Debug, Default)]
pub struct Pending;

impl Component for Pending {
    type Storage = NullStorage<Self>;
}

#[derive(Debug, Copy, Clone)]
pub enum Kind {
    // Reactors
    Electrolysis,
    /*
    // Power
    PowerSource,
    Pylon,
    // Other
    Storage,
    */
    Strut,
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
    #[allow(unused)]
    fn make(&self, world: &mut World, entity: Entity) {
        use self::Kind::*;
        match self {
            Electrolysis => Reactor::add(
                world, entity,
                /* input=  */ Pool::from(vec![(Resource::H2O, 2)]),
                /* delay=  */ REACTION_TIME,
                /* output= */ Pool::from(vec![(Resource::O2, 1), (Resource::H2, 2)]),
                /* power=  */ -3242.0,  // kJ/mol
                /* range=  */ REACTOR_RANGE,
            ),
            Strut => (),
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
    );

    fn run(&mut self, (lazy, entities, route_done, packets): Self::SystemData) {
        for (entity, _, packet) in (&*entities, &route_done, &packets).join() {
            entities.delete(entity).unwrap();
            let packet = packet.clone();
            lazy.exec_mut(move |world| {
                packet.kind.make(world, packet.target);
            });
        }
    }
}