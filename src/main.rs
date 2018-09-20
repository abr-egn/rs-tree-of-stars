extern crate hex2d;
extern crate amethyst;

mod geom;

use std::time::{Duration, Instant};

use hex2d::Coordinate;

use amethyst::input::{is_close_requested, is_key_down};
use amethyst::core::cgmath::{Vector3, Matrix4};
use amethyst::core::transform::{GlobalTransform, Transform, TransformBundle};
use amethyst::prelude::*;
use amethyst::renderer::{
    Camera, DisplayConfig, DrawFlat, Event, Pipeline, PosTex, Projection,
    RenderBundle, Stage, VirtualKeyCode
};

pub struct Main;

impl<'a, 'b> State<GameData<'a, 'b>> for Main {
    fn on_start(&mut self, data: StateData<GameData>) {
        initialize_cells(data.world);
        initialize_camera(data.world);
    }

    fn handle_event(&mut self, _: StateData<GameData>, event: Event) -> Trans<GameData<'a, 'b>> {
        if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
            Trans::Quit
        } else {
            Trans::None
        }
    }

    fn update(&mut self, data: StateData<GameData>) -> Trans<GameData<'a, 'b>> {
        data.data.update(&data.world);
        Trans::None
    }
}

const ARENA_HEIGHT: f32 = 100.0;
const ARENA_WIDTH: f32 = 100.0;

fn initialize_camera(world: &mut World) {
    // TODO: currently 0, 0 is lower-left; change transform so it's middle
    world.create_entity()
        .with(Camera::from(Projection::orthographic(
            0.0,
            ARENA_WIDTH,
            ARENA_HEIGHT,
            0.0,
        )))
        .with(GlobalTransform(
            Matrix4::from_translation(Vector3::new(0.0, 0.0, 1.0))
        ))
        .build();
}

fn initialize_cells(world: &mut World) {
    // TODO: remove these when they're auto-handled by setup
    world.register::<geom::Cell>();
    world.register::<geom::Speed>();
    world.register::<geom::Path>();

    // TODO: add sprites
    const ORIGIN: Coordinate = Coordinate { x: 0, y: 0 };
    const SPACING: hex2d::Spacing = hex2d::Spacing::FlatTop(10.0);

    let (x, y) = ORIGIN.to_pixel(SPACING);
    let mut tf = Transform::default();
    tf.translation = Vector3::new(x, y, 0.0);
    world.create_entity()
        .with(geom::Cell(ORIGIN))
        .with(GlobalTransform::default())
        .with(tf)
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
}

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let config = DisplayConfig::load("./resources/display_config.ron");
    let pipe = Pipeline::build().with_stage(
        Stage::with_backbuffer()
            .clear_target([0.0, 0.0, 0.0, 1.0], 1.0)
            .with_pass(DrawFlat::<PosTex>::new()),
    );
    let game_data = GameDataBuilder::default()
        .with_bundle(RenderBundle::new(pipe, Some(config)))?
        .with_bundle(TransformBundle::new())?;
    let mut game = Application::new("./", Main, game_data)?;
    game.run();

    Ok(())
}

/*
fn main() {
    let mut world = World::new();

    const TRAVEL: &str = "travel";
    const DRAW_CELLS: &str = "draw_cells";

    let mut dispatcher = DispatcherBuilder::new()
        .with(geom::Travel, TRAVEL, &[])
        .with(geom::DrawCells, DRAW_CELLS, &[TRAVEL])
        .build();

    dispatcher.setup(&mut world.res);

    const ORIGIN: Coordinate = Coordinate { x: 0, y: 0 };

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

    // This should be const but no const fn in stable yet.
    let frame_delay = Duration::new(1, 0) / 60;

    let mut quit = false;
    while !quit {
        let now = Instant::now();

        world.read_resource::<screen::Screen>().0.clear_screen();

        dispatcher.dispatch(&mut world.res);
        world.maintain();

        let screen = &world.read_resource::<screen::Screen>().0;
        screen.refresh().unwrap();
        let mut scr_read = screen.lock_read().unwrap();

        loop {
            let elapsed = now.elapsed();
            if elapsed >= frame_delay { break };
            let frame_timeout = frame_delay - elapsed;
            let ev = if let Some(ev) = scr_read.read_event(Some(frame_timeout)).unwrap() { ev } else { continue };
            use mortal::{Event::Key, Key::*};
            match ev {
                Key(Escape) => {
                    quit = true;
                    break;
                },
                _ => ()
            };
        }
    }
}
*/