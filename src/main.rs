extern crate specs;
extern crate hex2d;
extern crate mortal;
extern crate amethyst;

mod geom;
mod screen;

use std::time::{Duration, Instant};

use hex2d::Coordinate;
use specs::prelude::*;

use amethyst::input::{is_close_requested, is_key_down};
use amethyst::prelude::*;
use amethyst::renderer::{DisplayConfig, DrawFlat, Event, Pipeline,
                         PosTex, RenderBundle, Stage, VirtualKeyCode};

pub struct Main;

impl<'a, 'b> State<GameData<'a, 'b>> for Main {
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

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let config = DisplayConfig::load("./resources/display_config.ron");
    let pipe = Pipeline::build().with_stage(
        Stage::with_backbuffer()
            .clear_target([0.0, 0.0, 0.0, 1.0], 1.0)
            .with_pass(DrawFlat::<PosTex>::new()),
    );
    let game_data = GameDataBuilder::default()
        .with_bundle(RenderBundle::new(pipe, Some(config)))?;
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