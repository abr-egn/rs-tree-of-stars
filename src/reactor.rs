use std::time::Duration;

use hibitset::BitSet;
use rand::{self, Rng};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use crate::error::or_die;
use crate::geom;
use crate::graph;
use crate::power::Power;
use crate::resource::{self, Pool, Resource, Sink, Source};
use crate::util::{duration_f32, f32_duration};

#[derive(Debug)]
pub struct Progress {
    made: Option<ActiveProgress>,
}

#[derive(Debug)]
struct ActiveProgress {
    at: Duration,
    target: Duration,
    label: String,
}

impl Progress {
    pub fn new() -> Self { Progress { made: None} }
    pub fn at(&self) -> Option<f32> { self.at_label().map(|(p, _)| p) }
    pub fn at_label(&self) -> Option<(f32, &str)> {
        if let Some(active) = &self.made {
            Some((duration_f32(active.at) / duration_f32(active.target), &active.label))
        } else { None }
    }
    pub fn start(&mut self, target: Duration, label: String) {
        self.made = Some(ActiveProgress {
            at: Duration::new(0, 0),
            target, label,
        });
    }
    pub fn clear(&mut self) { self.made = None }
}

impl Component for Progress {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct MakeProgress;

impl<'a> System<'a> for MakeProgress {
    type SystemData = (
        WriteStorage<'a, Progress>,
        ReadStorage<'a, Power>,
    );

    fn run(&mut self, (mut progs, powers): Self::SystemData) {
        for (prog, opt_power) in (&mut progs, powers.maybe()).join() {
            let ActiveProgress { at, target, .. } = if let Some(p) = &mut prog.made { p } else { continue };
            if *at >= *target { continue }
            let ratio = opt_power.map_or(1.0, |power| {
                if power.total() >= 0.0 { 1.0 } else { power.ratio() }
            });
            // Duration doesn't support floating point mul/div :(
            let inc = f32_duration(duration_f32(super::UPDATE_DURATION)*ratio); 
            *at += inc;
        }
    }
}

#[derive(Debug)]
pub struct Reactor {
    input: Pool,
    delay: Duration,
    output: Pool,
    power_per_second: f32,
    targets: BitSet,
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
            
            world.write_storage().insert(entity, Power::new())?;
            world.write_storage().insert(entity, Progress::new())?;
            let power_per_second = total_power / duration_f32(delay);
            let mut targets = BitSet::new();
            for (r, _) in output.iter() { targets.add(r as u32); }
            world.write_storage().insert(entity, Reactor {
                input, delay, output, power_per_second, targets,
            })?;
            Ok(())
        });
    }
    pub fn input(&self) -> &Pool { &self.input }
    pub fn output(&self) -> &Pool { &self.output }
    #[allow(unused)]
    pub fn targets(&self) -> &BitSet { &self.targets }
    pub fn targets_mut(&mut self) -> &mut BitSet { &mut self.targets }
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
            if progress.at().map_or(false, |p| p >= 1.0) {
                progress.clear();
                power.clear::<Self>();
                for (res, count) in reactor.output.iter() {
                    if let Some(waste) = source.has.inc_by(res, count) {
                        spawn_waste(&lazy, node.at(), res, waste);
                    }
                }
            }

            // If nothing's in progress (or has just finished), start.
            if progress.made.is_some() { continue }
            let has_input = reactor.input.iter().all(|(r, c)| sink.has.get(r) >= c);
            if !has_input { continue }
            // TODO: make output gating controllable
            let needs_output = {
                let targets = &reactor.targets;
                reactor.output.iter().any(|(r, c)| {
                    targets.contains(r as u32) && source.has.get(r) < c
                })
            };
            if !needs_output { continue }
            // Start requesting power, and only continue if we're getting any.
            power.set::<Self>(reactor.power_per_second);
            if power.ratio() == 0.0 { continue }
            for (res, count) in reactor.input.iter() {
                if count == 0 { continue }
                sink.has.dec_by(res, count).unwrap();
            }
            progress.start(reactor.delay, "Reaction".into());
        }
    }
}

const WASTE_SPEED: f32 = 3.0;

fn spawn_waste(lazy: &LazyUpdate, center: ::hex2d::Coordinate, res: Resource, count: usize) {
    lazy.exec_mut(move |world| {
        let mut rng = rand::thread_rng();
        let targets = center.ring(5, hex2d::Spin::CW(hex2d::Direction::XY));
        for _ in 0..count {
            let ix: usize = rng.gen_range(0, targets.len());
            let target = targets[ix];
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