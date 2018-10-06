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
        kind: MWKind::Highlight,
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
            Event::KeyDown { keycode: Some(kc), .. } => {
                match kc {
                    Keycode::P => return EventAction::Push(PauseMode::new()),
                    Keycode::N => return EventAction::Push(PlaceMode::new()),
                    _ => (),
                }
            },
            _ => (),
        }
        EventAction::Done
    }
    fn on_top_event(&mut self, _: &mut World, _: &mut Context, event: Event) -> TopAction {
        match event {
            Event::KeyDown { keycode: Some(Keycode::N), .. } => {
                TopAction::Do(EventAction::Push(PlaceMode::new()))
            },
            _ => TopAction::AsEvent,
        }
    }
}

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
        world.write_resource::<MouseWidget>().kind = MWKind::Highlight;
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

/*
struct NodeSelected(Entity);

impl NodeSelected {
    fn new(node: Entity) -> Box<Mode> { Box::new(NodeSelected(node)) }
}

impl Mode for NodeSelected {

}
*/

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
    Highlight,
    PlaceNode,
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