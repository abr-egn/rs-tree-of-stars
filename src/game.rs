use ggez::{
    event::{Event, Keycode},
    graphics,
    Context,
};
use hex2d::{self, Coordinate};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use draw;
use geom;
use graph;
use mode::{Mode, EventAction, TopAction};
use resource::{self, Resource};

pub fn prep_world(world: &mut World) {
    world.add_resource(MouseWidget {
        coord: None,
        kind: MWKind::None,
    });
}

pub struct Play;

impl Play {
    pub fn new() -> Box<Mode> { Box::new(Play) }
}

impl Mode for Play {
    fn on_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> EventAction {
        match event {
            Event::MouseMotion { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                world.write_resource::<MouseWidget>().coord = Some(coord);
            },
            Event::KeyDown { keycode: Some(Keycode::P), .. } => {
                let p = &mut *world.write_resource::<super::Paused>();
                p.0 = !p.0;
            },
            _ => (),
        }
        EventAction::Done
    }
    fn on_top_event(&mut self, _: &mut World, _: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(kc), .. } => {
                match kc {
                    Keycode::N => TopAction::Do(EventAction::Push(PlaceNode::new())),
                    Keycode::S => TopAction::Do(EventAction::Push(Select::new())),
                    _ => TopAction::AsEvent,
                }
            },
            _ => TopAction::AsEvent,
        }
    }
}

struct Select;

impl Mode for Select {
    fn on_start(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<MouseWidget>().kind = MWKind::Highlight;
    }
    fn on_stop(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                match world.read_resource::<geom::Map>().get(coord) {
                    Some(ent) if world.read_storage::<graph::Node>().get(ent).is_some() => {
                        TopAction::Swap(NodeSelected::new(ent))
                    },
                    _ => TopAction::AsEvent,
                }
            },
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => TopAction::Pop,
            _ => TopAction::AsEvent,
        }
    }
}

impl Select {
    fn new() -> Box<Mode> { Box::new(Select) }
}

struct PlaceNode;

impl PlaceNode {
    fn new() -> Box<Mode> { Box::new(PlaceNode) }
}

impl Mode for PlaceNode {
    fn on_start(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<MouseWidget>().kind = MWKind::PlaceNode;
    }
    fn on_stop(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(Keycode::N), .. } |
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => TopAction::Pop,
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                if !graph::space_for_node(&*world.read_resource::<geom::Map>(), coord) {
                    return TopAction::Do(EventAction::Done)
                }
                graph::make_node(world, coord).unwrap();
                TopAction::Pop
            },
            _ => TopAction::AsEvent,
        }
    }
}

struct NodeSelected(Entity);

impl NodeSelected {
    fn new(node: Entity) -> Box<Mode> { Box::new(NodeSelected(node)) }
}

impl Mode for NodeSelected {
    fn on_start(&mut self, world: &mut World, _: &mut Context) {
        world.write_storage::<Selected>().insert(self.0, Selected).unwrap();
    }
    fn on_stop(&mut self, world: &mut World, _: &mut Context) {
        world.write_storage::<Selected>().remove(self.0);
    }
    fn on_top_event(&mut self, world: &mut World, _: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(kc), .. } => {
                match kc {
                    Keycode::Escape => TopAction::Swap(Select::new()),
                    Keycode::L => TopAction::push(PlaceLink::new(self.0)),
                    Keycode::G => {
                        GrowTest::start(world, self.0);
                        world.write_storage::<GrowTest>().get_mut(self.0).unwrap()
                            .next_growth = 0;
                        TopAction::done()
                    },
                    Keycode::S => {
                        if world.read_storage::<resource::Source>().get(self.0).is_some() {
                            return TopAction::done()
                        }
                        resource::Source::add(
                            world,
                            self.0,
                            resource::Pool::from(vec![(Resource::H2, 6)]),
                            10,
                        ).unwrap();
                        TopAction::done()
                    },
                    Keycode::D => {
                        let mut sinks = world.write_storage::<resource::Sink>();
                        if sinks.get(self.0).is_some() { return TopAction::done() }
                        let mut sink = resource::Sink::new();
                        sink.want.inc_by(Resource::H2, 6);
                        sinks.insert(self.0, sink).unwrap();
                        TopAction::done()
                    },
                    _ => TopAction::AsEvent,
                }
            },
            _ => TopAction::AsEvent,
        }
    }
}

struct PlaceLink(Entity);

impl PlaceLink {
    fn new(from: Entity) -> Box<Mode> { Box::new(PlaceLink(from)) }
}

impl Mode for PlaceLink {
    fn on_start(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<MouseWidget>().kind = MWKind::Highlight;
    }
    fn on_stop(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                let found = world.read_resource::<geom::Map>().get(coord);
                match found {
                    Some(ent) if ent != self.0 => {
                        let dest_at = if let Some(dest_node) = world.read_storage::<graph::Node>().get(ent) {
                            dest_node.at()
                        } else { return TopAction::AsEvent };
                        let self_at = world.read_storage::<graph::Node>().get(self.0).unwrap().at();
                        if !graph::space_for_link(&*world.read_resource(), self_at, dest_at) {
                            return TopAction::AsEvent
                        }
                        graph::make_link(world, self.0, ent).unwrap();
                        TopAction::Pop
                    },
                    _ => TopAction::AsEvent,
                }
            },
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => TopAction::Pop,
            _ => TopAction::AsEvent,
        }
    }

}

#[derive(Debug)]
pub struct MouseWidget {
    pub coord: Option<Coordinate>,
    pub kind: MWKind,
}

#[derive(Debug)]
pub enum MWKind {
    None,
    Highlight,
    PlaceNode,
}

#[derive(Debug, Default)]
pub struct Selected;

impl Component for Selected {
    type Storage = NullStorage<Self>;
}

#[derive(Debug)]
pub struct GrowTest {
    to_grow: Vec<hex2d::Direction>,
    next_growth: usize,
}

impl GrowTest {
    pub fn new() -> Self {
        GrowTest {
            to_grow: hex2d::Direction::all().iter().cloned().collect(),
            next_growth: 1,
        }
    }
    pub fn start(world: &mut World, ent: Entity) {
        {
            let mut grow = world.write_storage::<GrowTest>();
            if grow.get(ent).is_some() { return }
            grow.insert(ent, GrowTest::new()).unwrap();
        }
        resource::Source::add(world, ent,
            resource::Pool::from(vec![(Resource::H2, 6)]), 6)
            .unwrap();
        let mut sink = resource::Sink::new();
        sink.want.inc_by(Resource::H2, 6);
        world.write_storage::<resource::Sink>().insert(ent, sink).unwrap();
    }
}

impl Component for GrowTest {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug)]
pub struct RunGrowTest;

#[derive(SystemData)]
pub struct GrowTestData<'a> {
    entities: Entities<'a>,
    nodes: WriteStorage<'a, graph::Node>,
    grow: WriteStorage<'a, GrowTest>,
    sinks: WriteStorage<'a, resource::Sink>,
    lazy: Read<'a, LazyUpdate>,
}

const GROW_LEN: usize = 5;

impl<'a> System<'a> for RunGrowTest {
    type SystemData = GrowTestData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        let mut to_grow: Vec<(Entity, Coordinate, Coordinate)> = vec![];
        for (ent, node, sink, grow) in (&*data.entities, &mut data.nodes, &mut data.sinks, &mut data.grow).join() {
            if sink.has.get(Resource::H2) < grow.next_growth { continue }
            let next_dir = if let Some(d) = grow.to_grow.pop() { d } else { continue };
            let mut next_coord: Coordinate = node.at();
            for _ in 0..GROW_LEN {
                next_coord = next_coord + next_dir;
            }
            to_grow.push((ent, node.at(), next_coord));
            grow.next_growth += 1;
        }
        data.lazy.exec_mut(move |world| {
            for (from, at, next_coord) in to_grow {
                {
                    let map = &*world.read_resource::<geom::Map>();
                    if !graph::space_for_node(map, next_coord) { continue }
                    if !graph::space_for_link(map, at, next_coord) { continue }
                }
                let ent = graph::make_node(world, next_coord).unwrap();
                GrowTest::start(world, ent);
                graph::make_link(world, from, ent).unwrap();
            }
        })
    }
}

fn pixel_to_coord(ctx: &Context, mx: i32, my: i32) -> Coordinate {
    // TODO: there *has* to be a more direct way to do this - multiply by transform
    // matrix or something - but the types involved there are baffling.
    let rel_mx: f32 = (mx as f32) / (super::WINDOW_WIDTH as f32);
    let rel_my: f32 = (my as f32) / (super::WINDOW_HEIGHT as f32);
    let graphics::Rect { x, y, w, h } = graphics::get_screen_coordinates(ctx);
    let scr_mx: f32 = x + (w * rel_mx);
    let scr_my: f32 = y + (h * rel_my);
    Coordinate::from_pixel(scr_mx, scr_my, draw::SPACING)
}