use std::f32::consts::PI;

use ggez::{
    graphics::{self, DrawMode, Mesh, MeshBuilder, Point2},
    Context, GameResult,
};
use hex2d;
use specs::prelude::*;

use geom;

struct CellMesh(Mesh);

const HEX_SIDE: f32 = 10.0;
const SPACING: hex2d::Spacing = hex2d::Spacing::FlatTop(HEX_SIDE);

pub fn build_sprites(world: &mut World, ctx: &mut Context) -> GameResult<()> {
    let points: Vec<Point2> = (0..6).map(|ix| {
        let a = (PI / 3.0) * (ix as f32);
        Point2::new(a.cos(), a.sin()) * HEX_SIDE
    }).collect();
    let cell = MeshBuilder::new()
        .polygon(DrawMode::Fill, &points)
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
        ReadStorage<'a, geom::Shape>,
    );

    fn run(&mut self, (cell_mesh, shapes): Self::SystemData) {
        let ctx = &mut self.0;
        for &geom::Shape(ref coords) in shapes.join() {
            for coord in coords {
                let (x, y) = coord.to_pixel(SPACING);
                graphics::draw(ctx, &cell_mesh.0, Point2::new(x, y), 0.0).unwrap();
            }
        }
    }
}
