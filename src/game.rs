use ggez::{
    event::{Event, Keycode},
    graphics,
    Context,
};
use hex2d::{self, Coordinate};
use imgui::{ImGuiCond, Ui};
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use build;
use draw;
use error::or_die;
use geom;
use graph;
use mode::{Mode, EventAction, TopAction};
use power;
use reactor;
use resource::{self, Resource};
use util::*;

pub fn prep_world(world: &mut World) {
    world.add_resource(MouseWidget {
        coord: None,
        kind: MWKind::None,
        valid: true,
    });
}

pub struct Play;

impl Play {
    fn window<F: FnOnce(&mut World)>(&self, world: &mut World, ui: &Ui, f: F) -> Option<EventAction> {
        ui.window(im_str!("Play"))
            .always_auto_resize(true)
            .position((600.0, 100.0), ImGuiCond::FirstUseEver)
            .build(|| {
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
        None
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
        if let Some(ent) = handle_node_selection(world, ctx, &event) {
            TopAction::push(NodeSelected(ent))
        } else {
            TopAction::AsEvent
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

struct NodeSelected(Entity);

impl NodeSelected {
    fn window<F: FnOnce(&mut World)>(&self, world: &mut World, ui: &Ui, f: F) {
        ui.window(im_str!("Node")).always_auto_resize(true).build(|| {
            let mut kinds: Vec<String> = vec![];
            if world.read_storage::<reactor::Reactor>().get(self.0).is_some() {
                kinds.push("Reactor".into());
            }
            if world.read_storage::<power::Pylon>().get(self.0).is_some() {
                kinds.push("Pylon".into());
            }
            if world.read_storage::<power::Power>().get(self.0).is_some() {
                kinds.push("Power".into());
            }
            if world.read_storage::<build::Factory>().get(self.0).is_some() {
                kinds.push("Factory".into());
            }
            if kinds.is_empty() {
                kinds = vec!["None".into()];
            }
            ui.text(format!("Kind: {}", kinds.join(" | ")));
            if let Some(power) = world.read_storage::<power::Power>().get(self.0) {
                let total = power.total();
                let uses: Vec<String> = power.uses().map(|f| format!("{:+}", f)).collect();
                let uses_str = if uses.is_empty() { "None".into() } else { uses.join(" ") };
                ui.text(format!("Node Power: {}", uses_str));
                if total == 0.0 {
                    ui.text("Power Neutral");
                } else {
                    let dir = if total >= 0.0 { "Output" } else { "Input" };
                    ui.text(format!(
                        "Power {}: {:.0}% ({:+}/s of {:+}/s)", dir,
                        100.0*power.ratio(), power.grid(), power.total()));
                }
            }
            if let Some(prog) = world.read_storage::<reactor::Progress>().get(self.0) {
                if let Some(p) = prog.at() {
                    ui.text(format!("Progress: {:.0}%", 100.0*p));
                }
            }
            f(world);
        })
    }
}

/*
const REACTION_TIME: Duration = Duration::from_millis(5000);
const FACTORY_RANGE: i32 = 20;
*/

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
        let mut click = false;
        match event {
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => return TopAction::Pop,
            Event::MouseButtonDown { .. } => click = true,
            _ => (),
        };
        if let Some(ent) = handle_node_selection(world, ctx, &event) {
            TopAction::swap(NodeSelected(ent))
        } else {
            if click {
                TopAction::Pop
            } else {
                TopAction::AsEvent
            }
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
                action = TopAction::push(PlaceLink(self.0));
            }
            /*
            if self.is_plain(world) {
                use resource::Pool;
                if ui.small_button(im_str!("Make Reactor")) {
                    ui.open_popup(im_str!("Make Reactor"));
                }
                ui.popup(im_str!("Make Reactor"), || {
                    // Power is in kJ/mol
                    if ui.menu_item(im_str!("2H2O -> O2 + 2H2")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::from(vec![(Resource::H2O, 2)]),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::O2, 1), (Resource::H2, 2)]),
                            /* total_power= */ -3242.0,
                        );
                    }
                    if ui.menu_item(im_str!("C + O2 => CO2")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::from(vec![(Resource::C, 1), (Resource::O2, 1)]),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::CO2, 1)]),
                            /* total_power= */ 396.0,
                        );
                    }
                    if ui.menu_item(im_str!("CO2 + 4H2 => CH4 + 2H2O")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::from(vec![(Resource::CO2, 1), (Resource::H2, 4)]),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::CH4, 1), (Resource::H2O, 2)]),
                            /* total_power= */ 165.0,
                        );
                    }
                    if ui.menu_item(im_str!("CH4 + 2O2 => CO2 + 2H2O")).build() {
                        self.add_reactor(world,
                            /* input= */ Pool::from(vec![(Resource::CH4, 1), (Resource::O2, 2)]),
                            /* delay= */ REACTION_TIME,
                            /* output= */ Pool::from(vec![(Resource::CO2, 1), (Resource::H2O, 2)]),
                            /* total_power= */ 891.0,
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
                if ui.small_button(im_str!("Make Power Source")) {
                    world.write_storage().insert(self.0, power::Power::Source {
                        output: 100.0,
                    }).unwrap();
                }
                if ui.small_button(im_str!("Make Pylon")) {
                    power::Pylon::add(world, self.0);
                }
                if ui.small_button(im_str!("Make Factory")) {
                    or_die(|| {
                        world.write_storage().insert(self.0, build::Factory::new(
                            vec![build::Kind::Electrolysis, build::Kind::Strut]
                        ))?;
                        world.write_storage().insert(self.0, resource::Sink::new())?;
                        graph::AreaGraph::add(world, self.0, FACTORY_RANGE)?;
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
            */
            if world.read_storage::<graph::AreaGraph>().get(self.0).is_some() {
                ui.separator();
                if ui.small_button(im_str!("Toggle Exclude")) {
                    action = TopAction::push(ToggleExclude(self.0));
                }
            }
            if let Some(factory) = world.write_storage::<build::Factory>().get_mut(self.0) {
                ui.separator();
                let mut kinds: Vec<build::Kind> = factory.can_build().iter().cloned().collect();
                kinds.sort();
                for kind in kinds {
                    let name = format!("{:?}", kind);
                    ui.text(&name);
                    ui.same_line(100.0);
                    let built = factory.built(kind);
                    ui.text(format!("{}", built));
                    ui.same_line(115.0);
                    ui.push_id(&name);
                    if ui.small_button(im_str!("+")) {
                        factory.queue_push(kind);
                    }
                    if built > 0 {
                        ui.same_line(0.0);
                        if ui.small_button(im_str!("->")) {
                            action = TopAction::push(BuildFrom { source: self.0, kind });
                        }
                    }
                    ui.pop_id();
                }
                let queue = factory.queue();
                if !queue.is_empty() {
                    ui.separator();
                    for &kind in queue {
                        ui.text(format!("{:?}", kind));
                    }
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

struct BuildFrom {
    source: Entity,
    kind: build::Kind,
}

impl Mode for BuildFrom {
    fn name(&self) -> &str { "build from " }
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
                    Some(ent) => {
                        if world.read_storage::<graph::Node>().get(ent).is_some() {
                            TopAction::swap(BuildTo {
                                source: self.source,
                                kind: self.kind,
                                fork: ent,
                            })
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

struct BuildTo {
    source: Entity,
    kind: build::Kind,
    fork: Entity,
}

impl BuildTo {
    fn fork_coord(&self, world: &World) -> Coordinate {
        try_get(&world.read_storage::<graph::Node>(), self.fork).unwrap().at()
    }
    fn valid_to(&self, world: &World, coord: Coordinate) -> bool {
        let map = &*world.read_resource::<geom::Map>();
        if !graph::space_for_node(map, coord) {
            return false
        }
        if !graph::space_for_link(map, self.fork_coord(world), coord) {
            return false
        }
        true
    }
}

impl Mode for BuildTo {
    fn name(&self) -> &str { "build to" }
    fn on_show(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::PlaceNodeFrom(self.fork_coord(world));
    }
    fn on_hide(&mut self, world: &mut World) {
        world.write_resource::<MouseWidget>().kind = MWKind::None;
    }
    fn on_top_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> TopAction {
        match event {
            Event::MouseMotion { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                world.write_resource::<MouseWidget>().valid = self.valid_to(world, coord);
                TopAction::AsEvent
            },
            Event::MouseButtonDown { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                if !self.valid_to(world, coord) {
                    return TopAction::Do(EventAction::Done)
                }
                or_die(|| {
                    try_get_mut(&mut world.write_storage::<build::Factory>(), self.source)?
                        .dec_built(self.kind)?;
                    Ok(())
                });
                self.kind.start(world, self.source, self.fork, coord);
                TopAction::Pop
            },
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => TopAction::swap(BuildFrom {
                source: self.source, kind: self.kind,
            }),
            _ => TopAction::AsEvent,
        }
    }
}

struct PlaceLink(Entity);

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

impl Mode for ToggleExclude {
    fn name(&self) -> &str { "toggle exclude" }
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
                let found = if let Some(e) = world.read_resource::<geom::Map>().get(coord) { e }
                else { return TopAction::AsEvent };
                if found == self.0 { return TopAction::AsEvent };
                if world.read_storage::<graph::Node>().get(found).is_none() {
                    return TopAction::AsEvent;
                }
                let mut graphs = world.write_storage::<graph::AreaGraph>();
                let exclude = &mut or_die(|| try_get_mut(&mut graphs, self.0)).exclude_mut();
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
    pub valid: bool,
}

#[derive(Debug)]
pub enum MWKind {
    None,
    Highlight,
    PlaceNodeFrom(Coordinate),
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

fn handle_node_selection(world: &mut World, ctx: &Context, event: &Event) -> Option<Entity> {
    match *event {
        Event::MouseMotion { x, y, .. } => {
            let coord = pixel_to_coord(ctx, x, y);
            let valid = match world.read_resource::<geom::Map>().get(coord) {
                Some(ent) => world.read_storage::<graph::Node>().get(ent).is_some(),
                _ => true,
            };
            world.write_resource::<MouseWidget>().valid = valid;
            None
        },
        Event::MouseButtonDown { x, y, .. } => {
            let coord = pixel_to_coord(ctx, x, y);
            match world.read_resource::<geom::Map>().get(coord) {
                Some(ent) if world.read_storage::<graph::Node>().get(ent).is_some() => {
                    Some(ent)
                },
                _ => None,
            }
        },
        _ => None
    }
}