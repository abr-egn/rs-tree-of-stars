extern crate ggez;
extern crate hex2d;
extern crate specs;

mod draw;
mod geom;

use std::collections::HashSet;

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
        world.register::<geom::Link>();

        draw::build_sprites(&mut world, ctx)?;

        let update = DispatcherBuilder::new()
            //.with(geom::Travel, TRAVEL, &[])
            .build();

        const ORIGIN: Coordinate = Coordinate { x: 0, y: 0 };
        const SIDE: Coordinate = Coordinate { x: 12, y: -2 };

        let center_ent = world.create_entity()
            .with(geom::Shape(
                ORIGIN.ring(1, Spin::CW(Direction::XY))
            ))
            .with(geom::Source::new())
            .build();
        let side_ent = world.create_entity()
            .with(geom::Shape(
                SIDE.ring(1, Spin::CW(Direction::XY))
            ))
            .with(geom::Sink::new())
            .build();
        let link_path = ORIGIN.line_to(SIDE);
        let mut link_excl = HashSet::<Coordinate>::new();
        ORIGIN.for_each_in_range(1, |c| { link_excl.insert(c); });
        SIDE.for_each_in_range(1, |c| { link_excl.insert(c); });
        let link_ent = world.create_entity()
            .with(geom::Shape(link_path.iter().cloned()
                .filter(|c| !link_excl.contains(c))
                .collect()))
            .with(geom::Link {
                source: center_ent,
                sink: side_ent,
                path: link_path,
            })
            .build();
        
        geom::connect(
            world.write_storage::<geom::Source>(),
            world.write_storage::<geom::Sink>(),
            center_ent,
            side_ent,
            &[link_ent],
        )?;

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