use ggez::{
    event, graphics, timer,
    Context, GameResult,
};
use hex2d::Coordinate;

use draw;
use geom;

pub struct UI {
    pub main: super::Main,
    paused: bool,
}

impl UI {
    pub fn new(main: super::Main) -> Self {
        UI {
            main,
            paused: false,
        }
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
        graphics::clear(ctx);
        graphics::set_background_color(ctx, graphics::Color::new(0.0, 0.0, 0.0, 1.0));

        draw::draw(&mut self.main.world, ctx);

        graphics::present(ctx);
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
            P => self.paused = !self.paused,
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