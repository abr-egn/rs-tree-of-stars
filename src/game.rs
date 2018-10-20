use std::time::Duration;

use ggez::{
    event::{Event, Keycode},
    graphics,
    Context,
};
use hex2d::{self, Coordinate};
use imgui::Ui;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use draw;
use error::or_die;
use geom;
use graph;
use mode::{Mode, EventAction, TopAction};
use resource::{self, Resource};
use util::*;

pub fn prep_world(world: &mut World) {
    world.add_resource(MouseWidget {
        coord: None,
        kind: MWKind::None,
    });
}

pub struct Play;

impl Play {
    pub fn new() -> Box<Mode> { Box::new(Play) }
    fn window<F: FnOnce(&mut World)>(&self, world: &mut World, ui: &Ui, f: F) -> Option<EventAction> {
        let mut ret = None;
        ui.window(im_str!("Play")).always_auto_resize(true).build(|| {
            if ui.small_button(im_str!("Add Node")) {
                ret = Some(EventAction::Push(PlaceNode::new()));
            }
            ui.separator();
            {
                let p = &mut *world.write_resource::<super::Paused>();
                if p.0 {
                    if ui.small_button(im_str!("Unpause")) {
                        p.0 = false;
                    }
                } else {
                    if ui.small_button(im_str!("Pause")) {
                        p.0 = true;
                    }
                }
            }
            f(world);
        });
        ret
    }
}

impl Mode for Play {
    fn name(&self) -> &str { "play" }
    fn on_show(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::Highlight;
    }
    fn on_hide(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
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
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                match world.read_resource::<geom::Map>().get(coord) {
                    Some(ent) if world.read_storage::<graph::Node>().get(ent).is_some() => {
                        TopAction::push(NodeSelected::new(ent))
                    },
                    _ => TopAction::AsEvent,
                }
            },
            _ => TopAction::AsEvent,
        }
    }
    fn on_ui(&mut self, world: &mut World, ui: &Ui) -> EventAction {
        if let Some(ea) = self.window(world, ui, |_| {}) { return ea }
        EventAction::Done
    }
    fn on_top_ui(&mut self, world: &mut World, ui: &Ui) -> TopAction {
        let action = TopAction::done();
        if let Some(ea) = self.window(world, ui, |_| {
            // TODO: ???
        }) { return TopAction::Do(ea) }
        action
    }
}

struct PlaceNode;

impl PlaceNode {
    fn new() -> Box<Mode> { Box::new(PlaceNode) }
}

impl Mode for PlaceNode {
    fn name(&self) -> &str { "place node" }
    fn on_push(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::PlaceNode;
    }
    fn on_pop(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => TopAction::Pop,
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                if !graph::space_for_node(&*world.read_resource::<geom::Map>(), coord) {
                    return TopAction::Do(EventAction::Done)
                }
                graph::make_node(world, coord);
                TopAction::Pop
            },
            _ => TopAction::AsEvent,
        }
    }
}

struct NodeSelected(Entity);

impl NodeSelected {
    fn new(node: Entity) -> Box<Mode> { Box::new(NodeSelected(node)) }
    fn window<F: FnOnce(&mut World)>(&self, world: &mut World, ui: &Ui, f: F) {
        ui.window(im_str!("Node")).always_auto_resize(true).build(|| {
            let mut kinds: Vec<String> = vec![];
            if world.read_storage::<resource::Source>().get(self.0).is_some() {
                kinds.push("Source".into());
            }
            if world.read_storage::<resource::Sink>().get(self.0).is_some() {
                kinds.push("Sink".into());
            }
            if world.read_storage::<resource::Reactor>().get(self.0).is_some() {
                kinds.push("Reactor".into());
            }
            if kinds.is_empty() {
                ui.text("Kind: None");
            } else {
                ui.text(format!("Kind: {}", kinds.join(" | ")));
            }
            f(world);
        })
    }
    fn add_reactor(
        &self, world: &mut World,
        input: resource::Pool, delay: Duration, output: resource::Pool,
    ) {
        resource::Source::add(world, self.0, resource::Pool::new(), /* range= */ 20);
        or_die(|| {
            world.write_storage().insert(self.0, resource::Sink::new())?;
            world.write_storage().insert(self.0, resource::Reactor::new(input, delay, output))?;
            Ok(())
        });
    }
    fn is_plain(&self, world: &World) -> bool {
        if world.read_storage::<resource::Source>().get(self.0).is_some() { return false }
        if world.read_storage::<resource::Sink>().get(self.0).is_some() { return false }
        true
    }
}

const REACTION_TIME: Duration = Duration::from_millis(5000);

impl Mode for NodeSelected {
    fn name(&self) -> &str { "node selected" }
    fn on_push(&mut self, world: &mut World) {
        or_die(|| {
            world.write_storage::<Selected>().insert(self.0, Selected)?;
            Ok(())
        });
    }
    fn on_pop(&mut self, world: &mut World) {
        world.write_storage::<Selected>().remove(self.0);
    }
    fn on_show(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::Highlight;
    }
    fn on_hide(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => TopAction::Pop,
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                match world.read_resource::<geom::Map>().get(coord) {
                    Some(ent) if world.read_storage::<graph::Node>().get(ent).is_some() => {
                        TopAction::Swap(NodeSelected::new(ent))
                    },
                    _ => TopAction::Pop,
                }
            },
            _ => TopAction::AsEvent,
        }
    }
    fn on_ui(&mut self, world: &mut World, ui: &Ui) -> EventAction {
        self.window(world, ui, |_| {});
        EventAction::Continue
    }
    fn on_top_ui(&mut self, world: &mut World, ui: &Ui) -> TopAction {
        // AsEvent causes on_ui to get called, which causes the non-top widgets to be
        // double-added.
        let mut action = TopAction::continue_();
        self.window(world, ui, |world| {
            ui.separator();
            if ui.small_button(im_str!("Add Link")) {
                action = TopAction::push(PlaceLink::new(self.0));
            }
            if self.is_plain(world) {
                use resource::Pool;
                ui.menu(im_str!("Make Reactor")).build(|| {
                    if ui.menu_item(im_str!("-> H2O")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::new(),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::H2O, 1)]),
                        );
                    }
                    if ui.menu_item(im_str!("-> C")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::new(),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::C, 1)]),
                        );
                    }
                    if ui.menu_item(im_str!("2H2O -> O2 + 2H2")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::from(vec![(Resource::H2O, 2)]),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::O2, 1), (Resource::H2, 2)]),
                        );
                    }
                    if ui.menu_item(im_str!("C + O2 => CO2")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::from(vec![(Resource::C, 1), (Resource::O2, 1)]),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::CO2, 1)]),
                        );
                    }
                    if ui.menu_item(im_str!("CO2 + 4H2 => CH4 + 2H2O")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::from(vec![(Resource::CO2, 1), (Resource::H2, 4)]),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::CH4, 1), (Resource::H2O, 2)]),
                        );
                    }
                });
                if ui.small_button(im_str!("Make H2O Storage")) {
                    let mut pool = Pool::new();
                    for res in Resource::all() {
                        pool.set_cap(res, 0);
                    }
                    pool.set_cap(Resource::H2O, 6);
                    resource::Source::add(world, self.0, pool, /* range= */ 20);
                    or_die(|| {
                        world.write_storage().insert(self.0, resource::Sink::new())?;
                        world.write_storage().insert(self.0, resource::Storage)?;
                        Ok(())
                    });
                }
                if ui.small_button(im_str!("Make CH4 Burn")) {
                    let mut sink = resource::Sink::new();
                    sink.want.set(Resource::CH4, 1);
                    or_die(|| {
                        world.write_storage().insert(self.0, sink)?;
                        world.write_storage().insert(self.0, resource::Burn::new(REACTION_TIME))?;
                        Ok(())
                    });
                }
                ui.separator();
                if ui.small_button(im_str!("Start Growth Test")) {
                    GrowTest::start(world, self.0);
                    or_die(|| {
                        try_get_mut(&mut world.write_storage::<GrowTest>(), self.0)?.next_growth = 0;
                        Ok(())
                    });
                    action = TopAction::done()
                }
            }
            if world.read_storage::<resource::Source>().get(self.0).is_some() {
                if ui.small_button(im_str!("Toggle Exclude")) {
                    action = TopAction::push(ToggleExclude::new(self.0));
                }
            }
            ui.separator();
            if ui.small_button(im_str!("Deselect")) {
                action = TopAction::Pop;
            }
        });
        action
    }
}

struct PlaceLink(Entity);

impl PlaceLink {
    fn new(from: Entity) -> Box<Mode> { Box::new(PlaceLink(from)) }
}

impl Mode for PlaceLink {
    fn name(&self) -> &str { "place link" }
    fn on_show(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::Highlight;
    }
    fn on_hide(&mut self, world: &mut World) {
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
                        let self_at = {
                            let nodes = world.read_storage::<graph::Node>();
                            or_die(|| try_get(&nodes, self.0)).at()
                        };
                        if !graph::space_for_link(&*world.read_resource(), self_at, dest_at) {
                            return TopAction::AsEvent
                        }
                        graph::make_link(world, self.0, ent);
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

struct ToggleExclude(Entity);

impl ToggleExclude {
    fn new(node: Entity) -> Box<Mode> { Box::new(ToggleExclude(node)) }
}

impl Mode for ToggleExclude {
    fn name(&self) -> &str { "toggle exclude" }
    fn on_push(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::Highlight;
    }
    fn on_pop(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                let found = if let Some(e) = world.read_resource::<geom::Map>().get(coord) { e }
                else { return TopAction::AsEvent };
                if world.read_storage::<graph::Node>().get(found).is_none() {
                    return TopAction::AsEvent;
                }
                let mut sources = world.write_storage::<resource::Source>();
                let exclude = &mut or_die(|| try_get_mut(&mut sources, self.0)).exclude;
                if !exclude.remove(&found) { exclude.insert(found); }
                TopAction::Pop
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
        or_die(|| {
            {
                let mut grow = world.write_storage::<GrowTest>();
                if grow.get(ent).is_some() { return Ok(()) }
                grow.insert(ent, GrowTest::new())?;
            }
            resource::Source::add(world, ent,
                resource::Pool::from(vec![(Resource::H2, 6)]), 6);
            let mut sink = resource::Sink::new();
            sink.want.inc_by(Resource::H2, 6);
            world.write_storage::<resource::Sink>().insert(ent, sink)?;
            Ok(())
        });
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
                let ent = graph::make_node(world, next_coord);
                GrowTest::start(world, ent);
                graph::make_link(world, from, ent);
            }
        });
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