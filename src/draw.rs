use std::f32::consts::PI;

use ggez::{
    graphics::{
        self,
        Color, BlendMode, DrawMode, DrawParam, Mesh, MeshBuilder, Point2, Vector2,
    },
    Context, GameResult,
};
use specs::{
    prelude::*,
    Entities,
};

use geom;
use graph;
use resource;

struct CellMesh(Mesh);

struct PacketSprite {
    outline: Mesh,
    fill: Mesh,
}

impl graphics::Drawable for PacketSprite {
    fn draw_ex(&self, ctx: &mut Context, mut param: DrawParam) -> GameResult<()> {
        self.fill.draw_ex(ctx, param)?;
        param.color = Some(Color::new(1.0, 1.0, 1.0, 1.0));
        self.outline.draw_ex(ctx, param)?;
        Ok(())
    }
    fn set_blend_mode(&mut self, mode: Option<BlendMode>) {
        self.outline.set_blend_mode(mode);
        self.fill.set_blend_mode(mode);
    }
    fn get_blend_mode(&self) -> Option<BlendMode> { self.outline.get_blend_mode() }
}

pub fn build_sprites(world: &mut World, ctx: &mut Context) -> GameResult<()> {
    let points: Vec<Point2> = (0..6).map(|ix| {
        let a = (PI / 3.0) * (ix as f32);
        Point2::new(a.cos(), a.sin()) * super::HEX_SIDE
    }).collect();
    world.add_resource(CellMesh(Mesh::new_polygon(ctx, DrawMode::Fill, &points)?));

    let origin = Point2::new(0.0, 0.0);
    world.add_resource(PacketSprite {
        outline: Mesh::new_circle(ctx, DrawMode::Line(0.5), origin, 4.0, 0.5)?,
        fill: Mesh::new_circle(ctx, DrawMode::Fill, origin, 4.0, 0.5)?,
    });

    Ok(())
}

pub fn draw(world: &mut World, ctx: &mut Context) {
    graphics::clear(ctx);
    graphics::set_background_color(ctx, Color::new(0.0, 0.0, 0.0, 1.0));

    DrawCells(ctx).run_now(&mut world.res);
    DrawPackets(ctx).run_now(&mut world.res);
    DrawSources(ctx).run_now(&mut world.res);
    world.maintain();

    graphics::present(ctx);
}

struct DrawCells<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawCells<'b> {
    type SystemData = (
        ReadExpect<'a, CellMesh>,
        Entities<'a>,
        ReadStorage<'a, geom::Shape>,
        ReadStorage<'a, graph::Link>,
    );

    fn run(&mut self, (cell_mesh, entities, shapes, links): Self::SystemData) {
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

struct DrawSources<'a>(&'a mut Context);

const SOURCE_RADIUS: f32 = 30.0;

impl<'a, 'b> System<'a> for DrawSources<'b> {
    type SystemData = (
        ReadExpect<'a, PacketSprite>,
        ReadStorage<'a, geom::Center>,
        ReadStorage<'a, resource::Source>,
    );

    fn run(&mut self, (packet_sprite, centers, sources): Self::SystemData) {
        let ctx = &mut self.0;
        for (center, source) in (&centers, &sources).join() {
            let (x, y) = center.0.to_pixel(super::SPACING);
            let center_pt = Point2::new(x, y);
            if source.count == 0 { continue }
            let inc = (2.0*PI) / (source.count as f32);
            graphics::set_color(ctx, Color::new(0.0, 1.0, 0.0, 1.0)).unwrap();
            for ix in 0..source.count {
                let angle = (ix as f32) * inc;
                let v = Vector2::new(angle.cos(), angle.sin()) * SOURCE_RADIUS;
                graphics::draw(ctx, &*packet_sprite, center_pt + v, 0.0).unwrap();
            }
        }
    }
}

struct DrawPackets<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawPackets<'b> {
    type SystemData = (
        ReadExpect<'a, PacketSprite>,
        Entities<'a>,
        ReadStorage<'a, geom::Motion>,
        ReadStorage<'a, geom::MotionDone>,
    );

    fn run(&mut self, (packet_sprite, entities, motions, arrived): Self::SystemData) {
        let ctx = &mut self.0;
        for (entity, motion) in (&*entities, &motions).join() {
            let arrived = arrived.get(entity).is_some();
            let pos = motion.from + (motion.to - motion.from)*motion.at;
            graphics::set_color(ctx,
                if arrived { Color::new(1.0, 0.0, 1.0, 1.0) }
                else { Color::new(0.0, 0.0, 1.0, 1.0) }
            ).unwrap();
            graphics::draw(ctx, &*packet_sprite, pos, 0.0).unwrap();
        }
    }
}
