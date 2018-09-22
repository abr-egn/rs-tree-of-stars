extern crate hex2d;
extern crate specs;
extern crate ggez;

mod draw;
mod geom;
mod time;

use std::time::{Duration, Instant};

use hex2d::Coordinate;
use specs::prelude::*;
use ggez::{
    conf, event, graphics,
    Context, GameResult,
};

use time::{UpdateDelta, DrawDelta};

struct Main {
    world: World,
    update: Dispatcher<'static, 'static>,
    last_update: Instant,
    last_draw: Instant,
}

impl Main {
    fn new() -> GameResult<Self> {
        let mut world = World::new();

        const TRAVEL: &str = "travel";

        let mut update = DispatcherBuilder::new()
            .with(geom::Travel, TRAVEL, &[])
            .build();
        update.setup(&mut world.res);

        const ORIGIN: Coordinate = Coordinate { x: 0, y: 0 };

        world.add_resource(DrawDelta(Duration::new(0, 0)));  // TODO: remove
        world.create_entity()
            .with(geom::Cell(ORIGIN))
            .build();
        world.create_entity()
            .with(geom::Cell(Coordinate { x: 1, y: -1 }))
            .with(geom::Speed(2.0))
            .with(geom::Path {
                route: ORIGIN.ring(1, hex2d::Spin::CW(hex2d::Direction::XY)),
                index: 0,
                to_next: 0.0,
            })
            .build();

        let now = Instant::now();
        Ok(Main{ world, update, last_update: now, last_draw: now })
    }
}

impl event::EventHandler for Main {
    fn update(&mut self, _ctx: &mut Context) -> GameResult<()> {
        let now = Instant::now();
        self.world.write_resource::<UpdateDelta>().0 = now - self.last_update;
        self.last_update = now;

        self.update.dispatch(&mut self.world.res);
        self.world.maintain();
        
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        let now = Instant::now();
        self.world.write_resource::<DrawDelta>().0 = now - self.last_draw;
        self.last_draw = now;

        draw::draw(&mut self.world, ctx);

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
    let mut state = Main::new()?;
    graphics::set_screen_coordinates(&mut ctx, graphics::Rect {
        x: (WINDOW_WIDTH as f32) / -2.0,
        y: (WINDOW_HEIGHT as f32) / -2.0,
        w: WINDOW_WIDTH as f32,
        h: WINDOW_HEIGHT as f32,
    })?;
    event::run(&mut ctx, &mut state)?;

    Ok(())
}