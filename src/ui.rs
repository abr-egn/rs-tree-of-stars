use ggez::{
    event, graphics::{self, Point2, TextCached}, timer,
    Context, GameResult,
};
use hex2d::Coordinate;
use specs::{
    prelude::*,
    storage::BTreeStorage,
};

use draw;
use geom;

pub struct UI {
    pub main: super::Main,
    pause_text: Entity,
    paused: bool,
}

#[derive(Debug)]
pub struct TextWidget {
    pub text: TextCached,
    pub pos: Point2,
}

impl Component for TextWidget {
    type Storage = BTreeStorage<Self>;
}

#[derive(Debug, Default)]
pub struct ActiveWidget;

impl Component for ActiveWidget {
    type Storage = NullStorage<Self>;
}

impl UI {
    pub fn new(mut main: super::Main) -> GameResult<Self> {
        let pause_text = main.world.create_entity()
            .with(TextWidget {
                text: TextCached::new("PAUSED")?,
                pos: Point2::new(0.0, 0.0),
            })
            .build();
        Ok(UI {
            main,
            pause_text,
            paused: false,
        })
    }
}

impl event::EventHandler for UI {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        while timer::check_update_time(ctx, super::UPDATES_PER_SECOND) {
            if self.paused { continue }
            self.main.update();
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        draw::draw(&mut self.main.world, ctx);

        timer::yield_now();
        Ok(())
    }

    fn mouse_button_up_event(
        &mut self, ctx: &mut Context,
        _button: event::MouseButton, mx: i32, my: i32,
    ) {
        println!("Click at {}, {}", mx, my);
        let coord = pixel_to_coord(ctx, mx, my);
        println!("  => {:?}", coord);
        match self.main.world.read_resource::<geom::Map>().get(coord) {
            None => println!("  => nothin'"),
            Some(ent) => println!("  => {:?}", ent),
        }
    }

    fn mouse_motion_event(
        &mut self, ctx: &mut Context,
        _state: event::MouseState,
        mx: i32, my: i32, _xrel: i32, _yrel: i32,
    ) {
        let coord = pixel_to_coord(ctx, mx, my);
        *self.main.world.write_resource::<draw::MouseCoord>() = draw::MouseCoord(Some(coord));
    }

    fn key_down_event(
        &mut self, _: &mut Context,
        keycode: event::Keycode, _: event::Mod, _repeat: bool,
    ) {
        use event::Keycode::*;
        match keycode {
            P => {
                self.paused = !self.paused;
                let mut active = self.main.world.write_storage::<ActiveWidget>();
                if self.paused {
                    active.insert(self.pause_text, ActiveWidget).unwrap();
                } else {
                    active.remove(self.pause_text);
                }
            },
            _ => (),
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