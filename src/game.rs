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
                graph::make_node_world(world, coord).unwrap();
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
                    Keycode::L => TopAction::Do(EventAction::Push(PlaceLink::new(self.0))),
                    Keycode::G => {
                        GrowTest::start(
                            &mut world.write_storage(),
                            &mut world.write_storage(),
                            &mut world.write_storage(),
                            self.0,
                        );
                        world.write_storage::<GrowTest>().get_mut(self.0).unwrap()
                            .next_growth = 0;
                        TopAction::Do(EventAction::Done)
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
                        if world.read_storage::<graph::Node>().get(ent).is_some() &&
                            graph::space_for_link(&*world.read_resource(), &world.read_storage(), self.0, ent).unwrap() {
                            graph::make_link(world, self.0, ent).unwrap();
                            TopAction::Pop
                        } else {
                            TopAction::AsEvent
                        }
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
    pub fn start(
        grow: &mut WriteStorage<GrowTest>,
        sources: &mut WriteStorage<resource::Source>,
        sinks: &mut WriteStorage<resource::Sink>,
        ent: Entity,
    ) {
        if grow.get(ent).is_some() { return }
        grow.insert(ent, GrowTest::new()).unwrap();
        let mut source = resource::Source::new();
        source.has.inc_by(Resource::H2, 6);
        sources.insert(ent, source).unwrap();
        let mut sink = resource::Sink::new(20);
        sink.want.inc_by(Resource::H2, 6);
        sinks.insert(ent, sink).unwrap();
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
    graph: WriteExpect<'a, graph::Graph>,
    map: WriteExpect<'a, geom::Map>,
    spaces: WriteStorage<'a, geom::Space>,
    shapes: WriteStorage<'a, draw::Shape>,
    nodes: WriteStorage<'a, graph::Node>,
    grow: WriteStorage<'a, GrowTest>,
    sources: WriteStorage<'a, resource::Source>,
    sinks: WriteStorage<'a, resource::Sink>,
    links: WriteStorage<'a, graph::Link>,
}

const GROW_LEN: usize = 5;

impl<'a> System<'a> for RunGrowTest {
    type SystemData = GrowTestData<'a>;

    fn run(&mut self, mut data: Self::SystemData) {
        let mut to_grow: Vec<(Entity, Coordinate)> = vec![];
        for (ent, node, sink, grow) in (&*data.entities, &mut data.nodes, &mut data.sinks, &mut data.grow).join() {
            if sink.has.get(Resource::H2) < grow.next_growth { continue }
            let next_dir = if let Some(d) = grow.to_grow.pop() { d } else { continue };
            let mut next_coord: Coordinate = node.at();
            for _ in 0..GROW_LEN {
                next_coord = next_coord + next_dir;
            }
            to_grow.push((ent, next_coord));
            grow.next_growth += 1;
        }
        for (from, next_coord) in to_grow {
            if !graph::space_for_node(&*data.map, next_coord) { continue }
            let ent = graph::make_node(
                &data.entities,
                &mut *data.map,
                &mut data.spaces,
                &mut data.shapes,
                &mut data.nodes,
                next_coord,
            ).unwrap();
            GrowTest::start(&mut data.grow, &mut data.sources, &mut data.sinks, ent);
            if !graph::space_for_link(&*data.map, &data.nodes, from, ent).unwrap() { continue }
            graph::make_link_parts(
                &data.entities,
                &mut *data.graph,
                &mut *data.map,
                &mut data.spaces,
                &mut data.shapes,
                &mut data.links,
                &data.nodes,
                from, ent,
            ).unwrap();
        }
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