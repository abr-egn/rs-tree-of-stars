extern crate hex2d;
extern crate specs;
extern crate ggez;

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

struct DrawCells<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawCells<'b> {
    type SystemData = ReadStorage<'a, geom::Cell>;

    fn run(&mut self, cells: Self::SystemData) {
        const SPACING: hex2d::Spacing = hex2d::Spacing::FlatTop(10.0);
        let ctx = &mut self.0;
        for &geom::Cell(coord) in cells.join() {
            let (x, y) = coord.to_pixel(SPACING);
            graphics::circle(ctx,
                graphics::DrawMode::Fill,
                graphics::Point2::new(x + 400.0, y + 300.0),  // TODO: actual math
                10.0,
                1.0,
            ).unwrap();
        }
    }
}

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

        graphics::clear(ctx);
        graphics::set_background_color(ctx, graphics::Color::new(0.0, 0.0, 0.0, 1.0));

        DrawCells(ctx).run_now(&mut self.world.res);
        self.world.maintain();

        graphics::present(ctx);

        Ok(())
    }
}

fn main() -> GameResult<()> {
    let mut c = conf::Conf::default();
    c.window_setup.title = "Tree of Stars".to_owned();
    let mut ctx = Context::load_from_conf("Tree of Stars", "abe.egnor@gmail.com", c)?;
    let mut state = Main::new()?;
    event::run(&mut ctx, &mut state)?;

    Ok(())
}