extern crate ggez;
extern crate hex2d;
extern crate petgraph;
extern crate shred;
#[macro_use]
extern crate shred_derive;
extern crate specs;

mod draw;
mod geom;
mod graph;
mod map;
mod resource;
mod util;

use ggez::{
    conf, event, graphics, timer,
    Context, GameResult,
};
use hex2d::{Coordinate};
use specs::prelude::*;

use util::*;

pub const HEX_SIDE: f32 = 10.0;
pub const SPACING: hex2d::Spacing = hex2d::Spacing::FlatTop(HEX_SIDE);

pub const UPDATES_PER_SECOND: u32 = 60;
pub const UPDATE_DELTA: f32 = 1.0 / (UPDATES_PER_SECOND as f32);

struct Main {
    world: World,
    update: Dispatcher<'static, 'static>,
}

impl Main {
    fn new(ctx: &mut Context) -> GameResult<Self> {
        let mut world = World::new();

        world.register::<geom::Shape>();
        world.register::<geom::Motion>();
        world.register::<geom::MotionDone>();

        world.register::<map::Location>();

        world.register::<graph::Link>();
        world.register::<graph::Route>();
        world.register::<graph::RouteDone>();

        world.register::<resource::Source>();
        world.register::<resource::Sink>();
        world.register::<resource::Packet>();

        world.add_resource(map::Map::new());
        world.add_resource(graph::Graph::new());

        draw::build_sprites(&mut world, ctx)?;

        const TRAVEL: &str = "travel";
        const TRAVERSE: &str = "traverse";
        const PULL: &str = "pull";
        const RECEIVE: &str = "receive";
        let update = DispatcherBuilder::new()
            .with(geom::Travel, TRAVEL, &[])
            .with(graph::Traverse, TRAVERSE, &[TRAVEL])
            .with(resource::Pull, PULL, &[])
            .with(resource::Receive, RECEIVE, &[PULL])
            .build();

        let center_ent = graph::make_node(&mut world, Coordinate { x: 0, y: 0 });
        let mut source = resource::Source::new();
        source.has = 10;
        world.write_storage::<resource::Source>().insert(center_ent, source)
            .map_err(dbg)?;
        let side_ent = graph::make_node(&mut world, Coordinate { x: 12, y: -2 });
        let top_ent = graph::make_node(&mut world, Coordinate { x: 8, y: 10 });
        world.write_storage::<resource::Sink>().insert(top_ent, resource::Sink::new(5, 20))
            .map_err(dbg)?;
        
        graph::make_link(&mut world, center_ent, side_ent)?;
        graph::make_link(&mut world, top_ent, side_ent)?;

        Ok(Main{ world, update })
    }
}

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
    c.window_setup.samples = conf::NumSamples::Eight;
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