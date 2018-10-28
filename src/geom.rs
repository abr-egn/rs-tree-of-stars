use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
};

use ggez::{
    nalgebra,
    graphics::Point2,
};
use hex2d::Coordinate;
use spade::{
    self,
    rtree::RTree,
    BoundingRect,
};

use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use draw;
use graph;
use error::{Error, Result, or_die};
use util::*;

#[derive(Debug)]
pub struct Motion {
    pub from: Point2,
    pub to: Point2,
    pub inc: f32,
    pub at: f32,
}

impl Motion {
    pub fn new(from: Coordinate, to: Coordinate, speed: f32) -> Self {
        let (fx, fy) = from.to_pixel(draw::SPACING);
        let (tx, ty) = to.to_pixel(draw::SPACING);
        let from = Point2::new(fx, fy);
        let to = Point2::new(tx, ty);
        let dist = nalgebra::distance(&from, &to);
        /* Hex center to hex center is 2 * altitude of equilateral triangle */
        let speed_scale = 3.0f32.sqrt() * draw::HEX_SIDE;
        let inc = (speed * speed_scale * super::UPDATE_DELTA) / dist;
        Motion { from, to, inc, at: 0.0 }
    }
}

impl Component for Motion {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Default)]
pub struct MotionDone;

impl Component for MotionDone {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct Travel;

impl<'a> System<'a> for Travel {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, Motion>,
        WriteStorage<'a, MotionDone>,
    );

    fn run(&mut self, (entities, mut motions, mut arrived): Self::SystemData) {
        let mut v = Vec::new();
        for (entity, motion, ()) in (&*entities, &mut motions, !&arrived).join() {
            if motion.at >= 1.0 { continue };
            motion.at += motion.inc;
            if motion.at >= 1.0 {
                v.push(entity);
            }
        }
        or_die(|| {
            for entity in v {
                arrived.insert(entity, MotionDone)?;
            }
            Ok(())
        })
    }
}

#[derive(Debug)]
pub struct Space(HashSet<Coordinate>);

impl Space {
    pub fn new<T>(coords: T) -> Self
        where T: IntoIterator<Item=Coordinate>,
    { Space(coords.into_iter().collect()) }
    pub fn coords(&self) -> &HashSet<Coordinate> { &self.0 }
}

impl Component for Space {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct Map(HashMap<Coordinate, Entity>);

impl Map {
    pub fn new() -> Self { Map(HashMap::new()) }
    pub fn get(&self, coord: Coordinate) -> Option<Entity> { self.0.get(&coord).cloned() }
    pub fn is_occupied(&self, space: &Space) -> bool {
        space.coords().iter().any(|c| self.0.get(c).is_some())
    }
    pub fn set(
        &mut self, locs: &mut WriteStorage<Space>,
        ent: Entity, space: Space,
    ) -> Result<()> {
        if self.is_occupied(&space) {
            return Err(Error::Occupied)
        }
        let coords = space.0.clone();
        locs.insert(ent, space)?;
        for c in coords { self.0.insert(c, ent); }
        Ok(())
    }
    #[allow(unused)]
    pub fn clear(
        &mut self, locs: &mut WriteStorage<Space>,
        ent: Entity,
    ) -> Result<()> {
        {
            let space = try_get_mut(locs, ent)?;
            for c in space.coords() { self.0.remove(c); }
        }
        locs.remove(ent);
        Ok(())
    }
    pub fn in_range(&self, center: Coordinate, radius: i32) -> HashSet<Entity> {
        let mut out = HashSet::new();
        center.for_each_in_range(radius, |c| {
            if let Some(&e) = self.0.get(&c) {
                out.insert(e);
            }
        });
        out
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct SC(Coordinate);

impl spade::PointN for SC {
    type Scalar = i32;
    fn dimensions() -> usize { 2 }
    fn from_value(value: Self::Scalar) -> Self { SC(Coordinate::new(value, value))}
    fn nth(&self, index: usize) -> &Self::Scalar {
        match index {
            0 => &self.0.x,
            1 => &self.0.y,
            _ => panic!("invalid index"),
        }
    }
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        match index {
            0 => &mut self.0.x,
            1 => &mut self.0.y,
            _ => panic!("invalid index"),
        }
    }
}

impl spade::TwoDimensional for SC { }

#[derive(Debug, PartialEq)]
struct Area {
    center: Coordinate,
    radius: i32,
    bounds: BoundingRect<SC>,
    entity: Entity,
    typ: TypeId,
}

fn bounding(center: Coordinate, radius: i32) -> BoundingRect<SC> {
    let lower = Coordinate { x: center.x - radius, y: center.y - radius };
    let upper = Coordinate { x: center.x + radius, y: center.y + radius };
    BoundingRect::from_corners(&SC(lower), &SC(upper))
}

impl Area {
    fn new(center: Coordinate, radius: i32, entity: Entity, typ: TypeId) -> Self {
        Area { center, radius, bounds: bounding(center, radius), entity, typ }
    }
}

impl spade::SpatialObject for Area {
    type Point = SC;
    fn mbr(&self) -> BoundingRect<Self::Point> { self.bounds.mbr() }
    fn distance2(&self, point: &Self::Point) -> <Self::Point as spade::PointN>::Scalar {
        self.bounds.distance2(point)
    }
    fn contains(&self, point: &Self::Point) -> bool {
        self.bounds.contains(point)
    }
}

pub struct AreaMap(RTree<Area>);

impl AreaMap {
    pub fn new() -> Self { AreaMap(RTree::new()) }
    pub fn insert<T>(&mut self, center: Coordinate, radius: i32, entity: Entity)
        where T: 'static + ?Sized
    {
        self.0.insert(Area::new(center, radius, entity, TypeId::of::<T>()))
    }
    #[allow(unused)]
    pub fn remove<T>(&mut self, center: Coordinate, radius: i32, entity: Entity) -> bool
        where T: 'static + ?Sized
    {
        self.0.remove(&Area::new(center, radius, entity, TypeId::of::<T>()))
    }
    pub fn find<'a>(&'a self, at: Coordinate) -> impl Iterator<Item=Entity> + 'a {
        self.0.lookup_in_rectangle(&BoundingRect::from_point(SC(at)))
            .into_iter()
            .filter_map(move |area| {
                if area.center.distance(at) > area.radius {
                    return None
                }
                Some(area.entity)
            })
    }
    pub fn find_overlap<'a>(&'a self, center: Coordinate, radius: i32) -> impl Iterator<Item=Entity> + 'a {
        self.0.lookup_in_rectangle(&bounding(center, radius))
            .into_iter()
            .filter_map(move |area| {
                if area.center.distance(center) > area.radius + radius {
                    return None
                }
                Some(area.entity)
            })
    }
}

#[derive(Debug)]
pub struct AreaWatch<T> {
    range: i32,
    pub exclude: HashSet<Entity>,
    pub data: T,
}

pub struct AreaBuilder<T> {
    entity: Entity,
    range: i32,
    at: Coordinate,
    data: T,
}

impl<T> AreaBuilder<T> {
    pub fn insert(self: Self, world: &mut World) -> Result<()>
        where AreaWatch<T>: Component
    {
        let mut exclude = HashSet::new();
        exclude.insert(self.entity.clone());
        world.write_storage().insert(self.entity, AreaWatch {
            range: self.range, exclude, data: self.data,
        })?;
        world.write_resource::<AreaMap>().insert::<AreaWatch<T>>(self.at, self.range, self.entity);
        Ok(())
    }
}

impl<T> AreaWatch<T> {
    pub fn range(&self) -> i32 { self.range }
    pub fn exclude(&self) -> &HashSet<Entity> { &self.exclude }
    pub fn exclude_mut(&mut self) -> &mut HashSet<Entity> { &mut self.exclude }

    pub fn build<F: FnMut(&mut T, Entity)>(
        world: &World, entity: Entity, range: i32, mut data: T, mut f: F,
    ) -> Result<AreaBuilder<T>> {
        let at = try_get(&world.read_storage::<graph::Node>(), entity)?.at();
        for found in world.read_resource::<Map>().in_range(at, range) {
            f(&mut data, found);
        }
        Ok(AreaBuilder { entity, range, at, data })
    }
}

pub type AreaSet = AreaWatch<HashSet<Entity>>;

impl AreaSet {
    #[allow(unused)]
    pub fn add(world: &mut World, entity: Entity, range: i32) -> Result<()> {
        {
            let nodes = world.read_storage::<graph::Node>();
            Self::build(world, entity, range, HashSet::new(), |set, found| {
                if nodes.get(found).is_some() { set.insert(found); }
            })
        }?.insert(world)
    }
    #[allow(unused)]
    pub fn nodes<'a>(&'a self) -> impl Iterator<Item=Entity> + 'a { self.data.iter().cloned() }
}

impl Component for AreaSet {
    type Storage = BTreeStorage<Self>;
}