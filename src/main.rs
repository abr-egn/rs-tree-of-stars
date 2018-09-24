extern crate ggez;
extern crate hex2d;
extern crate specs;

mod draw;
mod game;
mod geom;

use std::{
    fmt::Debug,
};

use ggez::{
    conf, event, graphics, timer,
    Context, GameResult,
};
use hex2d::{Coordinate};
use specs::prelude::*;

struct Main {
    world: World,
    update: Dispatcher<'static, 'static>,
}

fn dbg<T: Debug>(t: T) -> String { format!("{:?}", t) }

impl Main {
    fn new(ctx: &mut Context) -> GameResult<Self> {
        let mut world = World::new();
        world.register::<geom::Shape>();
        world.register::<geom::Center>();
        world.register::<geom::Source>();
        world.register::<geom::Sink>();
        world.register::<geom::Link>();

        draw::build_sprites(&mut world, ctx)?;

        let update = DispatcherBuilder::new()
            //.with(geom::Travel, TRAVEL, &[])
            .build();

        let center_ent = game::make_node(&mut world, Coordinate { x: 0, y: 0 });
        world.write_storage().insert(center_ent, geom::Source::new()).map_err(dbg)?;

        let side_ent = game::make_node(&mut world, Coordinate { x: 12, y: -2 });
        world.write_storage().insert(side_ent, geom::Sink::new()).map_err(dbg)?;
        
        let link_ent = game::make_link(&mut world, center_ent, side_ent)?;
        
        game::connect(
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