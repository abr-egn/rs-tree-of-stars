use std::f32::consts::PI;

use ggez::{
    graphics::{
        self,
        Color, BlendMode, DrawMode, DrawParam, Mesh, Point2, TextCached, Vector2,
    },
    timer::get_time_since_start,
    Context, GameResult,
};
use hex2d::{Coordinate, Spacing};
use specs::{
    prelude::*,
};

use game;
use geom;
use graph;
use resource::{self, Resource};
use util;

pub const HEX_SIDE: f32 = 10.0;
pub const SPACING: Spacing = Spacing::FlatTop(HEX_SIDE);

#[derive(Debug)]
pub struct Shape {
    pub coords: Vec<Coordinate>,
    pub color: Color,
}

impl Component for Shape {
    type Storage = VecStorage<Self>;
}

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

struct OutlineSprite(Mesh);

struct PausedText(TextCached);

const PACKET_RADIUS: f32 = 4.0;

pub fn build_sprites(world: &mut World, ctx: &mut Context) -> GameResult<()> {
    let points: Vec<Point2> = (0..6).map(|ix| {
        let a = (PI / 3.0) * (ix as f32);
        Point2::new(a.cos(), a.sin()) * HEX_SIDE
    }).collect();
    world.add_resource(CellMesh(Mesh::new_polygon(ctx, DrawMode::Fill, &points)?));
    world.add_resource(OutlineSprite(Mesh::new_polygon(ctx, DrawMode::Line(1.0), &points)?));
    world.add_resource(PausedText(TextCached::new("PAUSED")?));

    let origin = Point2::new(0.0, 0.0);
    world.add_resource(PacketSprite {
        outline: Mesh::new_circle(ctx, DrawMode::Line(0.5), origin, PACKET_RADIUS, 0.5)?,
        fill: Mesh::new_circle(ctx, DrawMode::Fill, origin, PACKET_RADIUS, 0.5)?,
    });

    Ok(())
}

pub fn draw(world: &mut World, ctx: &mut Context) {
    graphics::clear(ctx);
    graphics::set_background_color(ctx, graphics::Color::new(0.0, 0.0, 0.0, 1.0));

    DrawCells(ctx).run_now(&mut world.res);
    DrawPackets(ctx).run_now(&mut world.res);
    DrawSources(ctx).run_now(&mut world.res);
    DrawSinks(ctx).run_now(&mut world.res);
    DrawMouseWidget(ctx).run_now(&mut world.res);
    DrawPaused(ctx).run_now(&mut world.res);
    world.maintain();

    graphics::present(ctx);
}

struct DrawCells<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawCells<'b> {
    type SystemData = (
        ReadExpect<'a, CellMesh>,
        ReadExpect<'a, OutlineSprite>,
        Entities<'a>,
        ReadStorage<'a, Shape>,
        ReadStorage<'a, game::Selected>,
    );

    fn run(&mut self, (cell_mesh, outline, entities, shapes, selected): Self::SystemData) {
        let ctx = &mut self.0;
        for (entity, shape) in (&*entities, &shapes).join() {
            graphics::set_color(ctx, shape.color).unwrap();
            for coord in &shape.coords {
                let (x, y) = coord.to_pixel(SPACING);
                graphics::draw(ctx, &cell_mesh.0, Point2::new(x, y), 0.0).unwrap();
            }
            if selected.get(entity).is_some() {
                let scale = (now_f32(ctx) * 3.0).sin() * 0.5 + 0.5;
                graphics::set_color(ctx, Color::new(scale, 0.0, scale, 1.0)).unwrap();
                for coord in &shape.coords {
                    let (x, y) = coord.to_pixel(SPACING);
                    graphics::draw(ctx, &outline.0, Point2::new(x, y), 0.0).unwrap();
                }
            }
        }
    }
}

fn now_f32(ctx: &Context) -> f32 { util::duration_f32(get_time_since_start(ctx)) }

fn draw_orbit(
    ctx: &mut Context, sprite: &PacketSprite, color: Color,
    orbit_radius: f32, orbit_speed: f32,
    coord: Coordinate, pool: &resource::Pool,
) -> GameResult<()> {
    let mut resources: Vec<(Resource, usize)> = vec![];
    for res in Resource::all() {
        let count = pool.get(res);
        if count > 0 {
            resources.push((res, count));
        }
    }
    if resources.len() == 0 { return Ok(()) }

    let orbit = (now_f32(ctx) * orbit_speed) % (2.0 * PI);
    let (x, y) = coord.to_pixel(SPACING);
    let center_pt = Point2::new(x, y);
    let inc = (2.0*PI) / (resources.len() as f32);
    graphics::set_color(ctx, color).unwrap();
    for ix in 0..resources.len() {
        let cluster_pt = {
            let angle = (ix as f32) * inc + orbit;
            let v = Vector2::new(angle.cos(), angle.sin()) * orbit_radius;
            center_pt + v
        };
        let count = resources[ix].1;
        let cluster_inc = (2.0*PI) / (count as f32);
        for px in 0..count {
            let angle = (px as f32) * cluster_inc;
            let v = Vector2::new(angle.cos(), angle.sin()) * PACKET_RADIUS;
            graphics::draw(ctx, sprite, cluster_pt + v, 0.0).unwrap();
        }
    }

    Ok(())
}

struct DrawSources<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawSources<'b> {
    type SystemData = (
        ReadExpect<'a, PacketSprite>,
        ReadStorage<'a, graph::Node>,
        ReadStorage<'a, resource::Source>,
    );

    fn run(&mut self, (packet_sprite, nodes, sources): Self::SystemData) {
        let ctx = &mut self.0;
        for (node, source) in (&nodes, &sources).join() {
            draw_orbit(
                ctx, &*packet_sprite, Color::new(0.0, 1.0, 0.0, 1.0),
                /* radius= */ 30.0, /* speed= */ 1.0,
                node.at(), &source.has,
            ).unwrap();
        }
    }
}

struct DrawSinks<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawSinks<'b> {
    type SystemData = (
        ReadExpect<'a, PacketSprite>,
        ReadStorage<'a, graph::Node>,
        ReadStorage<'a, resource::Sink>,
    );

    fn run(&mut self, (packet_sprite, nodes, sinks): Self::SystemData) {
        let ctx = &mut self.0;
        for (node, sink) in (&nodes, &sinks).join() {
            draw_orbit(
                ctx, &*packet_sprite, Color::new(0.0, 0.0, 0.0, 1.0),
                /* radius= */ 15.0, /* speed= */ -0.5,
                node.at(), &sink.has,
            ).unwrap();
        }
    }
}

struct DrawPackets<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawPackets<'b> {
    type SystemData = (
        ReadExpect<'a, PacketSprite>,
        ReadStorage<'a, geom::Motion>,
        ReadStorage<'a, resource::Packet>,
    );

    fn run(&mut self, (packet_sprite, motions, packets): Self::SystemData) {
        let ctx = &mut self.0;
        for (motion, _) in (&motions, &packets).join() {
            let pos = motion.from + (motion.to - motion.from)*motion.at;
            graphics::set_color(ctx, Color::new(0.0, 0.0, 1.0, 1.0)).unwrap();
            graphics::draw(ctx, &*packet_sprite, pos, 0.0).unwrap();
        }
    }
}

struct DrawMouseWidget<'a>(&'a mut Context);

impl <'a, 'b> System<'a> for DrawMouseWidget<'b> {
    type SystemData = (
        ReadExpect<'a, OutlineSprite>,
        ReadExpect<'a, CellMesh>,
        ReadExpect<'a, game::MouseWidget>,
        ReadExpect<'a, geom::Map>,
        ReadStorage<'a, geom::Space>,
    );

    fn run(&mut self, (outline, cell, mw, map, spaces): Self::SystemData) {
        let ctx = &mut self.0;

        let coord = if let Some(c) = mw.coord { c } else { return };
        match mw.kind {
            game::MWKind::None => (),
            game::MWKind::Highlight => {
                let coords = match map.get(coord) {
                    None => vec![coord],
                    Some(ent) => spaces.get(ent).unwrap().coords().iter().cloned().collect(),
                };
                graphics::set_color(ctx, Color::new(0.5, 0.5, 0.5, 1.0)).unwrap();
                for coord in coords {
                    let (x, y) = coord.to_pixel(SPACING);
                    graphics::draw(ctx, &outline.0, Point2::new(x, y), 0.0).unwrap();
                }
            },
            game::MWKind::PlaceNode => {
                let color = if graph::space_for_node(&*map, coord) {
                    Color::new(0.8, 0.8, 0.8, 0.5)
                } else {
                    Color::new(0.8, 0.0, 0.0, 0.5)
                };
                graphics::set_color(ctx, color).unwrap();
                for c in graph::node_shape(coord) {
                    let (x, y) = c.to_pixel(SPACING);
                    graphics::draw(ctx, &cell.0, Point2::new(x, y), 0.0).unwrap();
                }
            },
        }
    }
}

struct DrawPaused<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawPaused<'b> {
    type SystemData = (
        ReadExpect<'a, PausedText>,
        ReadExpect<'a, super::Paused>,
    );

    fn run(&mut self, (text, paused): Self::SystemData) {
        let ctx = &mut self.0;
        if paused.0 {
            graphics::set_color(ctx, Color::new(0.5, 1.0, 0.5, 1.0)).unwrap();
            graphics::draw(ctx, &text.0, Point2::new(0.0, 0.0), 0.0).unwrap();
        }
    }
}