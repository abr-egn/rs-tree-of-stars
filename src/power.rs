use std::collections::VecDeque;

use hibitset::BitSet;
use petgraph::{self, graphmap::GraphMap};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use error::or_die;
use geom;
use graph;
use util::try_get;

#[derive(Debug)]
pub enum Power {
    Source { output: f32 },
    Sink { need: f32, input: f32 },
}

impl Component for Power {
    type Storage = BTreeStorage<Self>;
}

pub struct PowerGrid {
    graph: GraphMap<Entity, (), petgraph::Undirected>,
}

impl PowerGrid {
    pub fn new() -> Self {
        PowerGrid {
            graph: GraphMap::new(),
        }
    }
    fn add_link(&mut self, from: Entity, to: Entity) {
        self.graph.add_edge(from, to, ());
    }
    fn find_covered(
        &self, areas: &ReadStorage<geom::AreaSet>,
        start: Entity, visited: &mut BitSet,
    ) -> BitSet {
        let mut pending = VecDeque::new();
        pending.push_back(start);
        visited.add(start.id());
        let mut covered = BitSet::new();
        while !pending.is_empty() {
            let pylon = pending.pop_front().unwrap();
            for n in self.graph.neighbors(pylon) {
                if !visited.add(n.id()) { pending.push_back(n) }
            }
            let area = if let Some(a) = areas.get(pylon) { a } else { continue };
            for entity in area.nodes() { covered.add(entity.id()); }
        }
        covered
    }
    pub fn links<'a>(&'a self, from: Entity) -> impl Iterator<Item=Entity> + 'a {
        self.graph.neighbors(from)
    }
}

#[derive(Debug)]
pub struct Pylon {
    range: i32,
}

impl Pylon {
    pub fn range(&self) -> i32 { self.range }
}

impl Component for Pylon {
    type Storage = BTreeStorage<Self>;
}

impl Pylon {
    pub fn add(world: &mut World, entity: Entity, range: i32) {
        or_die(|| {
            let at = try_get(&world.read_storage::<graph::Node>(), entity)?.at();
            {
                let map = world.read_resource::<geom::AreaMap>();
                let pylons = world.read_storage::<Pylon>();
                let found = map.find_overlap(at, range) & pylons.mask();
                let mut grid = world.write_resource::<PowerGrid>();
                for (other, _) in (&*world.entities(), found).join() {
                    grid.add_link(entity, other);
                }
            }
            geom::AreaSet::add(world, entity, range)?;
            world.write_storage().insert(entity, Pylon { range })?;
            Ok(())
        });
    }
}

#[derive(Debug)]
pub struct DistributePower;

#[derive(SystemData)]
pub struct DistributePowerData<'a> {
    entities: Entities<'a>,
    grid: ReadExpect<'a, PowerGrid>,
    areas: ReadStorage<'a, geom::AreaSet>,
    pylons: ReadStorage<'a, Pylon>,
    powers: WriteStorage<'a, Power>,
}

impl<'a> System<'a> for DistributePower {
    type SystemData = DistributePowerData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        let mut marked = BitSet::new();
        for (pylon, _) in (&*data.entities, &data.pylons).join() {
            if marked.contains(pylon.id()) { continue }
            let covered = data.grid.find_covered(&data.areas, pylon, &mut marked);
            let mut supply = 0.0;
            let mut demand = 0.0;
            for (power, _) in (&data.powers, &covered).join() {
                match power {
                    Power::Source { output } => supply += output,
                    Power::Sink { need, .. } => demand += need,
                }
            }
            let will_supply = fmin(supply, demand);
            let scale = if demand > 0.0 { will_supply / demand } else { 0.0 };
            for (power, _) in (&mut data.powers, covered).join() {
                match power {
                    Power::Source { .. } => (),
                    Power::Sink { need, input } => *input = *need * scale,
                }
            }
        }
    }
}

// std::cmp::min requires (total) Ord t(-_-t)
fn fmin(a: f32, b: f32) -> f32 {
    if a > b { b } else { a }
}

#[allow(unused)]
fn fmax(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}