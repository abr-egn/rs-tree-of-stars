extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate ggez;
extern crate hex2d;
extern crate petgraph;
extern crate shred;
#[macro_use]
extern crate shred_derive;
extern crate spade;
extern crate specs;

mod draw;
mod error;
mod game;
mod geom;
mod graph;
mod mode;
mod resource;
mod util;

use std::time::{Duration, Instant};

use ggez::{
    conf, event, graphics, timer,
    Context,
};
use specs::prelude::*;

use error::Result;

pub const UPDATES_PER_SECOND: u32 = 60;
pub const UPDATE_DELTA: f32 = 1.0 / (UPDATES_PER_SECOND as f32);
pub const UPDATE_DURATION: Duration = Duration::from_nanos(1_000_000_000 / (UPDATES_PER_SECOND as u64));

pub struct Now(pub Instant);
pub struct Paused(pub bool);

fn make_world(ctx: &mut Context) -> World {
    let mut world = World::new();

    world.register::<geom::Motion>();
    world.register::<geom::MotionDone>();
    world.register::<geom::Space>();

    world.register::<graph::Link>();
    world.register::<graph::Node>();
    world.register::<graph::AreaGraph>();
    world.register::<graph::FollowRoute>();
    world.register::<graph::RouteDone>();

    world.register::<resource::Source>();
    world.register::<resource::Sink>();
    world.register::<resource::Packet>();
    world.register::<resource::Reactor>();
    world.register::<resource::Storage>();

    world.register::<draw::Shape>();

    world.register::<game::Selected>();
    world.register::<game::GrowTest>();

    world.add_resource(Now(Instant::now()));
    world.add_resource(Paused(false));
    world.add_resource(geom::Map::new());
    world.add_resource(geom::AreaMap::new());

    draw::build_sprites(&mut world, ctx);
    game::prep_world(&mut world);

    /*
    let center_ent = graph::make_node_world(
        &mut world, Coordinate { x: 0, y: 0 })?;
    let mut source = resource::Source::new();
    source.has.set(Resource::H2, 6);
    source.has.set(Resource::O2, 6);
    world.write_storage().insert(center_ent, source)
        .map_err(dbg)?;
    world.write_storage().insert(center_ent, resource::Sink::new(20))
        .map_err(dbg)?;
    world.write_storage().insert(center_ent, resource::Reactor::new(
        resource::Pool::from(vec![
            (Resource::H2O, 2),
        ]),
        Duration::from_millis(5000),
        resource::Pool::from(vec![
            (Resource::H2, 2),
            (Resource::O2, 1),
        ]),
    )).map_err(dbg)?;
    
    let side_ent = graph::make_node_world(
        &mut world, Coordinate { x: 12, y: -2 })?;
    let top_ent = graph::make_node_world(
        &mut world, Coordinate { x: 8, y: 10 })?;
    world.write_storage().insert(top_ent, resource::Source::new())
        .map_err(dbg)?;
    world.write_storage().insert(top_ent, resource::Sink::new(20))
        .map_err(dbg)?;
    world.write_storage().insert(top_ent, resource::Reactor::new(
        resource::Pool::from(vec![
            (Resource::H2, 2),
            (Resource::O2, 1),
        ]),
        Duration::from_millis(5000),
        resource::Pool::from(vec![
            (Resource::H2O, 2),
        ]),
    )).map_err(dbg)?;
    
    graph::make_link(&mut world, center_ent, side_ent)?;
    graph::make_link(&mut world, top_ent, side_ent)?;
    */

    world
}

fn make_update() -> Dispatcher<'static, 'static> {
    const TRAVEL: &str = "travel";
    const TRAVERSE: &str = "traverse";
    const PULL: &str = "pull";
    const RECEIVE: &str = "receive";
    const REACTION: &str = "reaction";
    const STORAGE: &str = "storage";
    const GROW_TEST: &str = "grow_test";

    DispatcherBuilder::new()
        .with(geom::Travel, TRAVEL, &[])
        .with(graph::Traverse, TRAVERSE, &[TRAVEL])
        .with(resource::Pull, PULL, &[])
        .with(resource::Receive, RECEIVE, &[PULL])
        .with(resource::Reaction, REACTION, &[])
        .with(resource::DoStorage, STORAGE, &[])
        .with(game::RunGrowTest, GROW_TEST, &[])
        .build()
}

pub const WINDOW_WIDTH: u32 = 800;
pub const WINDOW_HEIGHT: u32 = 800;

fn main() -> Result<()> {
    let mut c = conf::Conf::default();
    c.window_setup.title = "Tree of Stars".to_owned();
    c.window_setup.samples = conf::NumSamples::Eight;
    c.window_mode.width = WINDOW_WIDTH;
    c.window_mode.height = WINDOW_HEIGHT;

    let mut ctx = Context::load_from_conf("Tree of Stars", "abe.egnor@gmail.com", c)?;
    graphics::set_screen_coordinates(&mut ctx, graphics::Rect {
        x: (WINDOW_WIDTH as f32) / -2.0,
        y: (WINDOW_HEIGHT as f32) / -2.0,
        w: WINDOW_WIDTH as f32,
        h: WINDOW_HEIGHT as f32,
    })?;
    let mut events = event::Events::new(&ctx)?;

    let mut world = make_world(&mut ctx);
    let mut update = make_update();
    let mut stack = mode::Stack::new();
    stack.push(&mut world, &mut ctx, game::Play::new());

    let mut running = true;
    while running {
        ctx.timer_context.tick();

        for event in events.poll() {
            ctx.process_event(&event);
            use event::Event;
            match event {
                Event::Quit { .. } => { running = false; break },
                ev => stack.handle(&mut world, &mut ctx, ev),
            }
        }
        if !running { break }

        while timer::check_update_time(&mut ctx, UPDATES_PER_SECOND) {
                if world.read_resource::<Paused>().0 { continue }
                world.write_resource::<Now>().0 += UPDATE_DURATION;
                update.dispatch(&mut world.res);
                world.maintain();
        }

        draw::draw(&mut world, &mut ctx);
        /*
        let mut count: usize = 0;
        for _ in world.read_storage::<graph::Node>().join() {
            count += 1;
        }
        println!("Nodes: {}", count);
        */
        timer::yield_now();
    }

    Ok(())
}