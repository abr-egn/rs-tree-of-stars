extern crate gfx;
extern crate gfx_device_gl;
extern crate ggez;
extern crate hex2d;
extern crate hibitset;
extern crate petgraph;
extern crate rand;
extern crate shred;
#[macro_use]
extern crate shred_derive;
extern crate spade;
extern crate specs;

#[macro_use]
extern crate imgui;
extern crate imgui_gfx_renderer;

mod draw;
mod error;
mod game;
mod geom;
mod ggez_imgui;
mod graph;
mod mode;
mod power;
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
    world.register::<geom::AreaSet>();

    world.register::<graph::Link>();
    world.register::<graph::Node>();
    world.register::<graph::AreaGraph>();
    world.register::<graph::FollowRoute>();
    world.register::<graph::RouteDone>();

    world.register::<resource::Source>();
    world.register::<resource::Sink>();
    world.register::<resource::Packet>();
    world.register::<resource::Target>();
    world.register::<resource::Reactor>();
    world.register::<resource::Storage>();
    world.register::<resource::Burn>();
    world.register::<resource::Waste>();

    world.register::<power::Power>();
    world.register::<power::Pylon>();

    world.register::<draw::Shape>();

    world.register::<game::Selected>();
    world.register::<game::GrowTest>();

    world.add_resource(Now(Instant::now()));
    world.add_resource(Paused(false));
    world.add_resource(geom::Map::new());
    world.add_resource(geom::AreaMap::new());
    world.add_resource(power::PowerGrid::new());

    draw::build_sprites(&mut world, ctx);
    game::prep_world(&mut world);

    world
}

fn make_update() -> Dispatcher<'static, 'static> {
    const TRAVEL: &str = "travel";
    const TRAVERSE: &str = "traverse";
    const PULL: &str = "pull";
    const RECEIVE: &str = "receive";
    const REACTION: &str = "reaction";
    const POWER: &str = "power";
    const STORAGE: &str = "storage";
    const BURN: &str = "burn";
    const GROW_TEST: &str = "grow_test";
    const CLEAR_WASTE: &str = "clear_waste";

    DispatcherBuilder::new()
        .with(geom::Travel, TRAVEL, &[])
        .with(graph::Traverse, TRAVERSE, &[TRAVEL])
        .with(resource::DoStorage, STORAGE, &[])
        .with(resource::Pull, PULL, &[STORAGE])
        .with(resource::Receive, RECEIVE, &[PULL])
        .with(power::DistributePower, POWER, &[])
        .with(resource::RunReactors, REACTION, &[RECEIVE, POWER])
        .with(resource::DoBurn, BURN, &[])
        .with(resource::ClearWaste, CLEAR_WASTE, &[])
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
    let mut ui_ctx = ggez_imgui::ImGuiContext::new(&mut ctx);

    let mut world = make_world(&mut ctx);
    let mut update = make_update();
    let mut stack = mode::Stack::new();
    stack.push(&mut world, game::Play::new());

    let mut running = true;
    while running {
        ctx.timer_context.tick();

        let mut ev_buffer = vec![];
        use event::Event;
        for event in events.poll() {
            ctx.process_event(&event);
            ui_ctx.process_event(&event);
            match event {
                Event::Quit { .. } => { running = false; break },
                _ => (),
            }
            ev_buffer.push(event);
        }
        if !running { break }

        let ui_frame = ui_ctx.frame(&mut ctx);
        for event in ev_buffer {
            match event {
                Event::MouseMotion { .. } |
                Event::MouseButtonDown { .. } |
                Event::MouseButtonUp { .. } |
                Event::MouseWheel { .. } => {
                    if ui_frame.ui.want_capture_mouse() { continue }
                },
                _ => (),
            }
            stack.handle_event(&mut world, &mut ctx, event);
        }

        while timer::check_update_time(&mut ctx, UPDATES_PER_SECOND) {
                if world.read_resource::<Paused>().0 { continue }
                world.write_resource::<Now>().0 += UPDATE_DURATION;
                update.dispatch(&mut world.res);
                world.maintain();
        }

        draw::draw(&mut world, &mut ctx);
        stack.handle_ui(&mut world, &ui_frame.ui);
        ui_frame.ui.show_demo_window(&mut true);

        ui_frame.render(&mut ctx);
        graphics::present(&mut ctx);

        /*
        let mut count: usize = 0;
        for _ in (&world.read_storage::<graph::Node>()).join() {
            count += 1;
        }
        println!("Nodes: {}", count);
        */

        timer::yield_now();
    }

    Ok(())
}