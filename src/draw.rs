use geom;

use ggez::{
    graphics,
    Context,
};
use hex2d;
use specs::prelude::*;

pub fn draw(world: &mut World, ctx: &mut Context) {
    graphics::clear(ctx);
    graphics::set_background_color(ctx, graphics::Color::new(0.0, 0.0, 0.0, 1.0));

    DrawCells(ctx).run_now(&mut world.res);
    world.maintain();

    graphics::present(ctx);
}

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
                graphics::Point2::new(x, y),
                10.0,
                1.0,
            ).unwrap();
        }
    }
}
