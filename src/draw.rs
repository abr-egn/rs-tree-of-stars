use geom;

use ggez::{
    graphics::{self, DrawMode, Mesh, MeshBuilder, Point2},
    Context, GameResult,
};
use hex2d;
use specs::prelude::*;

struct CellMesh(Mesh);

const SPACING: hex2d::Spacing = hex2d::Spacing::FlatTop(10.0);

pub fn build_sprites(world: &mut World, ctx: &mut Context) -> GameResult<()> {
    let cell = MeshBuilder::new()
        .circle(DrawMode::Fill, Point2::new(0.0, 0.0), 10.0, 1.0)
        .build(ctx)?;
    world.add_resource(CellMesh(cell));

    Ok(())
}

pub fn draw(world: &mut World, ctx: &mut Context) {
    graphics::clear(ctx);
    graphics::set_background_color(ctx, graphics::Color::new(0.0, 0.0, 0.0, 1.0));

    DrawCells(ctx).run_now(&mut world.res);
    world.maintain();

    graphics::present(ctx);
}

struct DrawCells<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawCells<'b> {
    type SystemData = (
        ReadExpect<'a, CellMesh>,
        ReadStorage<'a, geom::Cell>,
    );

    fn run(&mut self, (cell_mesh, cells): Self::SystemData) {
        let ctx = &mut self.0;
        for &geom::Cell(coord) in cells.join() {
            let (x, y) = coord.to_pixel(SPACING);
            graphics::draw(ctx, &cell_mesh.0, Point2::new(x, y), 0.0).unwrap();
        }
    }
}
