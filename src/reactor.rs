use std::time::Duration;

use rand::{self, Rng};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use error::or_die;
use geom;
use graph;
use power::Power;
use resource::{self, Pool, Resource, Sink, Source};
use util::{duration_f32, f32_duration};

#[derive(Debug)]
pub struct Reactor {
    input: Pool,
    delay: Duration,
    output: Pool,
    power_per_second: f32,
    in_progress: Option<Duration>,
}

impl Reactor {
    pub fn add(
        world: &mut World, entity: Entity,
        input: Pool, delay: Duration, output: Pool, total_power: f32, range: i32,
    ) {
        Source::add(world, entity, Pool::new(), range);
        or_die(|| {
            let mut sink = Sink::new();
            sink.want = input.clone();
            world.write_storage().insert(entity, sink)?;
            let power_per_second = total_power.abs() / duration_f32(delay);
            let power = if total_power >= 0.0 {
                Power::Source { output: 0.0 }
            } else {
                Power::Sink { need: 0.0, input: 0.0 }
            };
            world.write_storage().insert(entity, power)?;
            world.write_storage().insert(entity, Reactor {
                input, delay, output, power_per_second,
                in_progress: None,
            })?;
            Ok(())
        });
    }
    pub fn progress(&self) -> Option<f32> {
        let prog = if let Some(p) = &self.in_progress { p } else { return None };
        Some(duration_f32(*prog) / duration_f32(self.delay))
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
        WriteStorage<'a, Power>,
        Read<'a, LazyUpdate>,
    );

    fn run(&mut self, (nodes, mut reactors, mut sources, mut sinks, mut powers, lazy): Self::SystemData) {
        for (node, reactor, source, sink, power) in (&nodes, &mut reactors, &mut sources, &mut sinks, &mut powers).join() {
            // Check in progress production.
            let produce = if let Some(prog) = reactor.in_progress.as_mut() {
                let inc = match power {
                    Power::Source { .. } => super::UPDATE_DURATION,
                    Power::Sink { input, .. } => {
                        let ratio = *input / reactor.power_per_second;
                        // Duration doesn't support floating point mul/div :(
                        f32_duration(duration_f32(super::UPDATE_DURATION)*ratio)
                    },
                };
                *prog += inc;
                *prog >= reactor.delay
            } else { false };
            if produce {
                reactor.in_progress = None;
                for (res, count) in reactor.output.iter() {
                    if let Some(waste) = source.has.inc_by(res, count) {
                        spawn_waste(&lazy, node.at(), res, waste);
                    }
                }
                match power {
                    Power::Source { output } => *output = 0.0,
                    Power::Sink { need, .. } => *need = 0.0,
                }
            }

            // If nothing's in progress (or has just finished), start.
            if reactor.in_progress.is_some() { continue }
            let has_input = reactor.input.iter().all(|(r, c)| sink.has.get(r) >= c);
            if !has_input { continue }
            let needs_output = reactor.output.iter().any(|(r, c)| source.has.get(r) < c);
            if !needs_output { continue }
            // TODO?: start reaction if there's power demand
            let power_start = match power {
                Power::Source { output } => {
                    *output = reactor.power_per_second;
                    true
                },
                Power::Sink { need, input } => {
                    *need = reactor.power_per_second;
                    *input > 0.0
                },
            };
            if !power_start { continue }
            for (res, count) in reactor.input.iter() {
                if count == 0 { continue }
                sink.has.dec_by(res, count).unwrap();
            }
            reactor.in_progress = Some(Duration::new(0, 0));
        }
    }
}

const WASTE_SPEED: f32 = 3.0;

fn spawn_waste(lazy: &LazyUpdate, center: ::hex2d::Coordinate, res: Resource, count: usize) {
    lazy.exec_mut(move |world| {
        let mut rng = rand::thread_rng();
        let targets = center.ring(5, hex2d::Spin::CW(hex2d::Direction::XY));
        for _ in 0..count {
            let target = targets[rng.gen_range::<usize>(0, targets.len())];
            world.create_entity()
                .with(resource::Packet { resource: res })
                .with(geom::Motion::new(center, target, WASTE_SPEED))
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