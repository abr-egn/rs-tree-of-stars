use std::{
    any::TypeId,
    collections::{HashMap, VecDeque},
};

use hibitset::BitSet;
use petgraph::{self, graphmap::GraphMap};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use crate::error::or_die;
use crate::geom;
use crate::graph;
use crate::util::try_get;

#[derive(Debug)]
pub struct Power {
    has: HashMap<TypeId, f32>,
    from_grid: f32,
}

impl Power {
    pub fn new() -> Self { Power { has: HashMap::new(), from_grid: 0.0 } }
    pub fn set<T: 'static>(&mut self, amount: f32) -> Option<f32> {
        self.has.insert(TypeId::of::<T>(), amount)
    }
    pub fn clear<T: 'static>(&mut self) -> Option<f32> {
        self.has.remove(&TypeId::of::<T>())
    }
    pub fn total(&self) -> f32 {
        self.has.values().sum()
    }
    pub fn from_grid(&self) -> f32 { self.from_grid }
    pub fn ratio(&self) -> f32 {
        let total = self.total();
        if total == 0.0 { 1.0 }
        else { self.from_grid / self.total() }
    }
    pub fn uses(&self) -> impl Iterator<Item=f32> {
        let mut items: Vec<(TypeId, f32)> = self.has.iter().map(|(&k, &v)| (k, v)).collect();
        items.sort_by_key(|(k, _)| *k);
        items.into_iter().map(|(_, v)| v)
    }
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

#[derive(shred_derive::SystemData)]
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
                let total = power.total();
                if total >= 0.0 {
                    supply += total
                } else {
                    demand += total.abs()
                }
            }
            let will_supply = fmin(supply, demand);
            let (in_scale, out_scale) = if demand > 0.0 && supply > 0.0 {
                (will_supply / demand, will_supply / supply)
            } else { (0.0, 0.0) };
            for (power, _) in (&mut data.powers, covered).join() {
                let total = power.total();
                if total < 0.0 {
                    power.from_grid = total * in_scale
                } else {
                    power.from_grid = total * out_scale
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