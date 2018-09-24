extern crate ggez;
extern crate hex2d;
extern crate specs;

mod draw;
mod geom;

use ggez::{
    conf, event, graphics, timer,
    Context, GameResult,
};
use hex2d::{Coordinate, Direction, Spin};
use specs::prelude::*;

struct Main {
    world: World,
    update: Dispatcher<'static, 'static>,
}

impl Main {
    fn new(ctx: &mut Context) -> GameResult<Self> {
        let mut world = World::new();
        world.register::<geom::Shape>();
        world.register::<geom::Source>();
        world.register::<geom::Sink>();

        draw::build_sprites(&mut world, ctx)?;

        const TRAVEL: &str = "travel";

        let mut update = DispatcherBuilder::new()
            //.with(geom::Travel, TRAVEL, &[])
            .build();

        const ORIGIN: Coordinate = Coordinate { x: 0, y: 0 };

        world.create_entity()
            .with(geom::Shape(
                ORIGIN.ring(1, Spin::CW(Direction::XY))
            ))
            .build();
        world.create_entity()
            .with(geom::Shape(
                Coordinate { x: 12, y: -2 }.ring(1, Spin::CW(Direction::XY))
            ))
            /*
            .with(geom::Speed(1.0))
            .with(geom::Path {
                route: ORIGIN.ring(1, hex2d::Spin::CW(hex2d::Direction::XY)),
                index: 0,
                to_next: 0.0,
            })
            */
            .build();

        Ok(Main{ world, update })
    }
}

pub const UPDATES_PER_SECOND: u32 = 60;

impl event::EventHandler for Main {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        while timer::check_update_time(ctx, UPDATES_PER_SECOND) {
            self.update.dispatch(&mut self.world.res);
            self.world.maintain();
        }
        
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        draw::draw(&mut self.world, ctx);

        timer::yield_now();
        Ok(())
    }
}

fn main() -> GameResult<()> {
    const WINDOW_WIDTH: u32 = 800;
    const WINDOW_HEIGHT: u32 = 800;

    let mut c = conf::Conf::default();
    c.window_setup.title = "Tree of Stars".to_owned();
    c.window_mode.width = WINDOW_WIDTH;
    c.window_mode.height = WINDOW_HEIGHT;

    let mut ctx = Context::load_from_conf("Tree of Stars", "abe.egnor@gmail.com", c)?;
    let mut state = Main::new(&mut ctx)?;
    graphics::set_screen_coordinates(&mut ctx, graphics::Rect {
        x: (WINDOW_WIDTH as f32) / -2.0,
        y: (WINDOW_HEIGHT as f32) / -2.0,
        w: WINDOW_WIDTH as f32,
        h: WINDOW_HEIGHT as f32,
    })?;
    event::run(&mut ctx, &mut state)?;

    Ok(())
}