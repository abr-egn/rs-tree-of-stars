use std::f32::consts::PI;

use ggez::{
    graphics::{self, Color, DrawMode, Mesh, MeshBuilder, Point2},
    Context, GameResult,
};
use hex2d;
use specs::{
    prelude::*,
    Entities,
};

use geom;

struct CellMesh(Mesh);

pub fn build_sprites(world: &mut World, ctx: &mut Context) -> GameResult<()> {
    let points: Vec<Point2> = (0..6).map(|ix| {
        let a = (PI / 3.0) * (ix as f32);
        Point2::new(a.cos(), a.sin()) * super::HEX_SIDE
    }).collect();
    let cell = MeshBuilder::new()
        .polygon(DrawMode::Fill, &points)
        .build(ctx)?;
    world.add_resource(CellMesh(cell));

    Ok(())
}

pub fn draw(world: &mut World, ctx: &mut Context) {
    graphics::clear(ctx);
    graphics::set_background_color(ctx, Color::new(0.0, 0.0, 0.0, 1.0));

    DrawCells(ctx).run_now(&mut world.res);
    DrawPackets(ctx).run_now(&mut world.res);
    world.maintain();

    graphics::present(ctx);
}

struct DrawCells<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawCells<'b> {
    type SystemData = (
        Entities<'a>,
        ReadExpect<'a, CellMesh>,
        ReadStorage<'a, geom::Shape>,
        ReadStorage<'a, geom::Link>,
    );

    fn run(&mut self, (entities, cell_mesh, shapes, links): Self::SystemData) {
        let ctx = &mut self.0;
        for (entity, &geom::Shape(ref coords)) in (&*entities, &shapes).join() {
            graphics::set_color(ctx, if links.get(entity).is_some() {
                Color::new(0.0, 1.0, 0.0, 1.0)
            } else {
                Color::new(1.0, 1.0, 1.0, 1.0)
            }).unwrap();
            for coord in coords {
                let (x, y) = coord.to_pixel(super::SPACING);
                graphics::draw(ctx, &cell_mesh.0, Point2::new(x, y), 0.0).unwrap();
            }
        }
    }
}

struct DrawPackets<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawPackets<'b> {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, geom::Motion>,
        ReadStorage<'a, geom::Arrived>,
    );

    fn run(&mut self, (entities, motions, arrived): Self::SystemData) {
        let ctx = &mut self.0;
        for (entity, motion) in (&*entities, &motions).join() {
            let arrived = arrived.get(entity).is_some();
            let pos = motion.from + (motion.to - motion.from)*motion.at;
            graphics::set_color(ctx,
                if arrived { Color::new(1.0, 0.0, 1.0, 1.0) }
                else { Color::new(0.0, 1.0, 1.0, 1.0) }
            ).unwrap();
            graphics::circle(ctx, DrawMode::Fill, pos, 4.0, 0.5).unwrap();
        }
    }
}

/*
impl<'a, 'b> System<'a> for DrawPackets<'b> {
    type SystemData = (
        ReadStorage<'a, geom::Link>,
        ReadStorage<'a, geom::Packet>,
    );

    fn run(&mut self, (links, packets): Self::SystemData) {
        let ctx = &mut self.0;
        /*
        for packet in (&packets).join() {
            if packet.done() { continue };
            let link = if let Some(l) = links.get(packet.route[packet.route_index]) { l } else { continue };
            // TODO: lerp between prev, current, and next
            let (x, y) = link.path[packet.path_index].to_pixel(SPACING);
            graphics::set_color(ctx, Color::new(0.0, 0.0, 1.0, 1.0)).unwrap();
            graphics::circle(ctx, DrawMode::Fill, Point2::new(x, y), 4.0, 0.5).unwrap();
        }
        */
    }
}
*/
