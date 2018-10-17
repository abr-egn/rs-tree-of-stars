use std::f32::consts::PI;

use ggez::{
    self,
    graphics::{
        self,
        Color, BlendMode, DrawMode, DrawParam, Mesh, Point2, TextCached, Vector2,
    },
    timer::get_time_since_start,
    Context,
};
use hex2d::{Coordinate, Spacing};
use specs::{
    prelude::*,
};

use error::or_die;
use game;
use geom;
use graph;
use resource::{self, Resource};
use util::{self, try_get};

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
    fn draw_ex(&self, ctx: &mut Context, mut param: DrawParam) -> ggez::GameResult<()> {
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

struct SourceOrbit(Mesh);

pub struct ModeText(TextCached);

impl ModeText {
    pub fn set(&mut self, s: &str) {
        or_die(|| { self.0 = TextCached::new(s)?; Ok(()) })
    }
}

const PACKET_RADIUS: f32 = 4.0;

pub fn build_sprites(world: &mut World, ctx: &mut Context) {
    let points: Vec<Point2> = (0..6).map(|ix| {
        let a = (PI / 3.0) * (ix as f32);
        Point2::new(a.cos(), a.sin()) * HEX_SIDE
    }).collect();
    or_die(|| {
        world.add_resource(CellMesh(Mesh::new_polygon(ctx, DrawMode::Fill, &points)?));
        world.add_resource(OutlineSprite(Mesh::new_polygon(ctx, DrawMode::Line(1.0), &points)?));
        world.add_resource(PausedText(TextCached::new("PAUSED")?));
        world.add_resource(SourceOrbit(Mesh::new_circle(ctx,
            DrawMode::Line(1.0),
            Point2::new(0.0, 0.0),
            source_radius(),
            /* tolerance= */ 0.5,
        )?));
        let origin = Point2::new(0.0, 0.0);
        world.add_resource(PacketSprite {
            outline: Mesh::new_circle(ctx, DrawMode::Line(0.5), origin, PACKET_RADIUS, 0.5)?,
            fill: Mesh::new_circle(ctx, DrawMode::Fill, origin, PACKET_RADIUS, 0.5)?,
        });
        world.add_resource(ModeText(TextCached::new("<INVALID>")?));
        Ok(())
    })
}

pub fn draw(world: &mut World, ctx: &mut Context) {
    graphics::clear(ctx);
    graphics::set_background_color(ctx, graphics::Color::new(0.0, 0.0, 0.0, 1.0));

    DrawCells(ctx).run_now(&mut world.res);
    DrawPackets(ctx).run_now(&mut world.res);
    DrawSources(ctx).run_now(&mut world.res);
    DrawSinks(ctx).run_now(&mut world.res);
    DrawReactors(ctx).run_now(&mut world.res);
    DrawMouseWidget(ctx).run_now(&mut world.res);
    DrawText(ctx).run_now(&mut world.res);
    world.maintain();

    //graphics::present(ctx);
}

trait ToPixelPoint {
    fn to_pixel_point(&self) -> Point2;
}

impl ToPixelPoint for Coordinate {
    fn to_pixel_point(&self) -> Point2 {
        let (x, y) = self.to_pixel(SPACING);
        Point2::new(x, y)
    }
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
        let screen = graphics::get_screen_coordinates(ctx);
        or_die(|| {
            for (entity, shape) in (&*entities, &shapes).join() {
                graphics::set_color(ctx, shape.color)?;
                for coord in &shape.coords {
                    let p = coord.to_pixel_point();
                    if !screen.contains(p) { continue }
                    graphics::draw(ctx, &cell_mesh.0, p, 0.0)?;
                }
                if selected.get(entity).is_some() {
                    let scale = (now_f32(ctx) * 3.0).sin() * 0.5 + 0.5;
                    graphics::set_color(ctx, Color::new(scale, 0.0, scale, 1.0))?;
                    for coord in &shape.coords {
                        let p = coord.to_pixel_point();
                        if !screen.contains(p) { continue }
                        graphics::draw(ctx, &outline.0, p, 0.0)?;
                    }
                }
            }
            Ok(())
        })
    }
}

fn now_f32(ctx: &Context) -> f32 { util::duration_f32(get_time_since_start(ctx)) }

fn res_color(res: Resource) -> Color {
    match res {
        Resource::H2 => Color::new(1.0, 1.0, 0.0, 1.0),
        Resource::O2 => Color::new(0.0, 1.0, 0.0, 1.0),
        Resource::H2O => Color::new(0.0, 0.0, 1.0, 1.0),
        Resource::C => Color::new(0.7, 0.7, 0.7, 1.0),
        Resource::CO2 => Color::new(1.0, 0.0, 1.0, 1.0),
        Resource::CH4 => Color::new(1.0, 0.5, 0.0, 1.0),
    }
}

fn draw_orbit(
    ctx: &mut Context, screen: graphics::Rect, sprite: &PacketSprite,
    orbit_radius: f32, orbit_speed: f32,
    coord: Coordinate, pool: &resource::Pool,
) {
    let mut resources: Vec<(Resource, usize)> = vec![];
    for res in Resource::all() {
        let count = pool.get(res);
        if count > 0 {
            resources.push((res, count));
        }
    }
    if resources.len() == 0 { return }

    let orbit = (now_f32(ctx) * orbit_speed) % (2.0 * PI);
    let center_pt = coord.to_pixel_point();
    let inc = (2.0*PI) / (resources.len() as f32);
    or_die(|| {
        for ix in 0..resources.len() {
            let cluster_pt = {
                let angle = (ix as f32) * inc + orbit;
                let v = Vector2::new(angle.cos(), angle.sin()) * orbit_radius;
                center_pt + v
            };
            let count = resources[ix].1;
            let cluster_inc = (2.0*PI) / (count as f32);
            graphics::set_color(ctx, res_color(resources[ix].0))?;
            for px in 0..count {
                let angle = (px as f32) * cluster_inc;
                let v = Vector2::new(angle.cos(), angle.sin()) * PACKET_RADIUS * 1.5;
                let final_point = cluster_pt + v;
                if !screen.contains(final_point) { continue }
                graphics::draw(ctx, sprite, final_point, 0.0)?;
            }
        }
        Ok(())
    })
}

/* Should be const */
fn source_radius() -> f32 { 3.0f32.sqrt() * HEX_SIDE * 2.0 }

struct DrawSources<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawSources<'b> {
    type SystemData = (
        ReadExpect<'a, PacketSprite>,
        ReadExpect<'a, SourceOrbit>,
        ReadStorage<'a, graph::Node>,
        ReadStorage<'a, resource::Source>,
    );

    fn run(&mut self, (packet_sprite, source_orbit, nodes, sources): Self::SystemData) {
        let ctx = &mut self.0;
        let screen = graphics::get_screen_coordinates(ctx);
        for (node, source) in (&nodes, &sources).join() {
            let pt = node.at().to_pixel_point();
            if screen.contains(pt) {
                or_die(|| {
                    graphics::set_color(ctx, Color::new(1.0, 1.0, 1.0, 1.0))?;
                    graphics::draw(ctx, &source_orbit.0, pt, 0.0)?;
                    Ok(())
                });
            }
            draw_orbit(
                ctx, screen, &*packet_sprite,
                /* radius= */ source_radius(), /* speed= */ 1.0,
                node.at(), &source.has,
            );
        }
    }
}

#[derive(PartialEq, Eq)]
enum SinkState {
    Green,
    Yellow,
    Red,
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
        let screen = graphics::get_screen_coordinates(ctx);
        for (node, sink) in (&nodes, &sinks).join() {
            let pt = node.at().to_pixel_point();
            if screen.contains(pt) {
                let mut state = SinkState::Green;
                for (res, want) in sink.want.iter() {
                    let has = sink.has.get(res);
                    if has >= want { continue }
                    if has + sink.in_transit.get(res) >= want {
                        if state == SinkState::Green {
                            state = SinkState::Yellow;
                        }
                    } else {
                        state = SinkState::Red;
                    }
                }
                let color = match state {
                    SinkState::Green => Color::new(0.0, 1.0, 0.0, 1.0),
                    SinkState::Yellow => Color::new(1.0, 1.0, 0.0, 1.0),
                    SinkState::Red => Color::new(1.0, 0.0, 0.0, 1.0),
                };
                or_die(|| {
                    graphics::set_color(ctx, color)?;
                    graphics::draw(ctx, &*packet_sprite, pt, 0.0)?;
                    Ok(())
                });
            }
            draw_orbit(
                ctx, screen, &*packet_sprite,
                /* radius= */ 3.0f32.sqrt() * HEX_SIDE, /* speed= */ -0.5,
                node.at(), &sink.has,
            );
        }
    }
}

struct DrawReactors<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawReactors<'b> {
    type SystemData = (
        ReadStorage<'a, graph::Node>,
        ReadStorage<'a, resource::Reactor>,
    );

    fn run(&mut self, (nodes, reactors): Self::SystemData) {
        let ctx = &mut self.0;
        let screen = graphics::get_screen_coordinates(ctx);
        or_die(|| { graphics::set_color(ctx, Color::new(1.0, 0.0, 1.0, 1.0))?; Ok(()) });
        for (node, reactor) in (&nodes, &reactors).join() {
            let progress = if let Some(p) = reactor.progress() { p } else { continue };
            let pt = node.at().to_pixel_point();
            if !screen.contains(pt) { continue }
            or_die(|| {
                graphics::circle(ctx, DrawMode::Line(3.0), pt, source_radius() * progress, 0.5)?;
                Ok(())
            });
        }
    }
}

struct DrawPackets<'a>(&'a mut Context);

const WASTE_SCALE: f32 = 0.5;

impl<'a, 'b> System<'a> for DrawPackets<'b> {
    type SystemData = (
        Entities<'a>,
        ReadExpect<'a, PacketSprite>,
        ReadStorage<'a, geom::Motion>,
        ReadStorage<'a, resource::Packet>,
        ReadStorage<'a, resource::Waste>,
    );

    fn run(&mut self, (entities, packet_sprite, motions, packets, waste): Self::SystemData) {
        let ctx = &mut self.0;
        let screen = graphics::get_screen_coordinates(ctx);
        for (entity, motion, packet) in (&*entities, &motions, &packets).join() {
            let pos = motion.from + (motion.to - motion.from)*motion.at;
            if !screen.contains(pos) { continue }
            let is_waste = waste.get(entity).is_some();
            or_die(|| {
                graphics::set_color(ctx, res_color(packet.resource))?;
                graphics::draw(ctx, &*packet_sprite, pos, 0.0)?;
                if is_waste {
                    graphics::set_color(ctx, Color::new(1.0, 0.0, 0.0, 1.0))?;
                    let up_l = pos + (Vector2::new(-HEX_SIDE, -HEX_SIDE) * WASTE_SCALE);
                    let up_r = pos + (Vector2::new(HEX_SIDE, -HEX_SIDE) * WASTE_SCALE);
                    let dn_l = pos + (Vector2::new(-HEX_SIDE, HEX_SIDE) * WASTE_SCALE);
                    let dn_r = pos + (Vector2::new(HEX_SIDE, HEX_SIDE) * WASTE_SCALE);
                    graphics::line(ctx, &[up_l, dn_r], 1.0)?;
                    graphics::line(ctx, &[up_r, dn_l], 1.0)?;
                }
                Ok(())
            });
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
        or_die(|| { match mw.kind {
            game::MWKind::None => (),
            game::MWKind::Highlight => {
                let coords = match map.get(coord) {
                    None => vec![coord],
                    Some(ent) => try_get(&spaces, ent)?.coords().iter().cloned().collect(),
                };
                graphics::set_color(ctx, Color::new(0.5, 0.5, 0.5, 1.0))?;
                for coord in coords {
                    let (x, y) = coord.to_pixel(SPACING);
                    graphics::draw(ctx, &outline.0, Point2::new(x, y), 0.0)?;
                }
            },
            game::MWKind::PlaceNode => {
                let color = if graph::space_for_node(&*map, coord) {
                    Color::new(0.8, 0.8, 0.8, 0.5)
                } else {
                    Color::new(0.8, 0.0, 0.0, 0.5)
                };
                graphics::set_color(ctx, color)?;
                for c in graph::node_shape(coord) {
                    let (x, y) = c.to_pixel(SPACING);
                    graphics::draw(ctx, &cell.0, Point2::new(x, y), 0.0)?;
                }
            },
        }; Ok(()) })
    }
}

struct DrawText<'a>(&'a mut Context);

impl<'a, 'b> System<'a> for DrawText<'b> {
    type SystemData = (
        ReadExpect<'a, ModeText>,
        ReadExpect<'a, PausedText>,
        ReadExpect<'a, super::Paused>,
    );

    fn run(&mut self, (mode_text, paused_text, is_paused): Self::SystemData) {
        let ctx = &mut self.0;
        or_die(|| {
            graphics::set_color(ctx, Color::new(0.5, 1.0, 0.5, 1.0))?;
            if is_paused.0 {
                graphics::draw(ctx, &paused_text.0, Point2::new(0.0, 0.0), 0.0)?;
            }
            graphics::draw(ctx, &mode_text.0, Point2::new(0.0, 780.0), 0.0)?;
            Ok(())
        });
    }
}