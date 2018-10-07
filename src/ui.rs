use ggez::{
    event::{Event, Keycode},
    graphics::{self, Point2, TextCached},
    Context,
};
use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use draw;
use geom;
use graph;
use mode::{Mode, EventAction, TopAction};

pub fn prep_world(world: &mut World) {
    world.add_resource(MouseWidget {
        coord: None,
        kind: MWKind::None,
    });
}

pub struct PlayMode;

impl PlayMode {
    pub fn new() -> Box<Mode> { Box::new(PlayMode) }
}

impl Mode for PlayMode {
    fn on_event(&mut self, world: &mut World, ctx: &mut Context, event: Event) -> EventAction {
        match event {
            Event::MouseMotion { x, y, .. } => {
                let coord = pixel_to_coord(ctx, x, y);
                world.write_resource::<MouseWidget>().coord = Some(coord);
            },
            Event::KeyDown { keycode: Some(Keycode::P), .. } => {
                return EventAction::Push(PauseMode::new())
            },
            _ => (),
        }
        EventAction::Done
    }
    fn on_top_event(&mut self, _: &mut World, _: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(kc), .. } => {
                match kc {
                    Keycode::N => TopAction::Do(EventAction::Push(PlaceMode::new())),
                    Keycode::S => TopAction::Do(EventAction::Push(FindSelection::new())),
                    _ => TopAction::AsEvent,
                }
            },
            _ => TopAction::AsEvent,
        }
    }
}

struct FindSelection;

impl Mode for FindSelection {
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

impl FindSelection {
    fn new() -> Box<Mode> { Box::new(FindSelection) }
}

// TODO: get rid of this as a mode, and just make it behavior in PlayMode
struct PauseMode {
    widget: Option<Entity>,
}

impl PauseMode {
    fn new() -> Box<Mode> { Box::new(PauseMode { widget: None }) }
}

impl Mode for PauseMode {
    fn on_start(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<super::Paused>().0 = true;
        let ent = world.create_entity()
            .with(TextWidget {
                text: TextCached::new("PAUSED").unwrap(),
                pos: Point2::new(0.0, 0.0),
            })
            .build();
        self.widget = Some(ent);
    }
    fn on_stop(&mut self, world: &mut World, _: &mut Context) {
        world.write_resource::<super::Paused>().0 = false;
        world.delete_entity(self.widget.unwrap()).unwrap();
    }
    fn on_event(&mut self, _: &mut World, _: &mut Context, event: Event) -> EventAction {
        match event {
            Event::MouseMotion { .. } => EventAction::Continue,
            _ => EventAction::Done,
        }
    }
    fn on_top_event(&mut self, _: &mut World, _: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(Keycode::P), .. } => TopAction::Pop,
            _ => TopAction::AsEvent,
        }
    }
}

struct PlaceMode;

impl PlaceMode {
    fn new() -> Box<Mode> { Box::new(PlaceMode) }
}

impl Mode for PlaceMode {
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
    fn on_top_event(&mut self, _: &mut World, _: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(kc), .. } => {
                match kc {
                    Keycode::Escape => TopAction::Swap(FindSelection::new()),
                    Keycode::L => TopAction::Do(EventAction::Push(PlaceLink::new(self.0))),
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
                            graph::space_for_link(world, self.0, ent).unwrap() {
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
pub struct TextWidget {
    pub text: TextCached,
    pub pos: Point2,
}

impl Component for TextWidget {
    type Storage = BTreeStorage<Self>;
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