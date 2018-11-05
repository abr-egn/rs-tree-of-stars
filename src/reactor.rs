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
pub struct Progress {
    made: Option<Duration>,
    target: Duration,
    power_per_second: f32,
}

impl Progress {
    pub fn at(&self) -> Option<f32> {
        let prog = if let Some(p) = &self.made { p } else { return None };
        Some(duration_f32(*prog) / duration_f32(self.target))
    }
}

impl Component for Progress {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct MakeProgress;

impl<'a> System<'a> for MakeProgress {
    type SystemData = (
        WriteStorage<'a, Progress>,
        WriteStorage<'a, Power>,
    );

    fn run(&mut self, (mut progs, mut powers): Self::SystemData) {
        for (prog, power) in (&mut progs, &mut powers).join() {
            let made = if let Some(m) = &mut prog.made { m } else { continue };
            if *made >= prog.target { continue }
            let inc = match power {
                Power::Source { .. } => super::UPDATE_DURATION,
                Power::Sink { input, .. } => {
                    let ratio = *input / prog.power_per_second;
                    // Duration doesn't support floating point mul/div :(
                    f32_duration(duration_f32(super::UPDATE_DURATION)*ratio)
                },
            };
            *made += inc;
        }
    }
}

#[derive(Debug)]
pub struct Reactor {
    input: Pool,
    delay: Duration,
    output: Pool,
    power_per_second: f32,
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
            world.write_storage().insert(entity, Progress {
                made: None, target: delay, power_per_second,
            })?;
            world.write_storage().insert(entity, Reactor {
                input, delay, output, power_per_second,
            })?;
            Ok(())
        });
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
        WriteStorage<'a, Progress>,
        WriteStorage<'a, Source>,
        WriteStorage<'a, Sink>,
        WriteStorage<'a, Power>,
        Read<'a, LazyUpdate>,
    );

    fn run(&mut self, (nodes, mut reactors, mut progs, mut sources, mut sinks, mut powers, lazy): Self::SystemData) {
        for (node, reactor, progress, source, sink, power) in (&nodes, &mut reactors, &mut progs, &mut sources, &mut sinks, &mut powers).join() {
            // Check in progress production.
            let produce = if let Some(p) = progress.at() { p >= 1.0 } else { false };
            if produce {
                progress.made = None;
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
            if progress.made.is_some() { continue }
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
            progress.made = Some(Duration::new(0, 0));
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