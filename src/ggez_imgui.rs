use gfx;
use gfx_device_gl;
use ggez::{
    Context,
    graphics, timer,
    event::{Event, MouseButton},
};

use imgui::{
    self,
    ImGui, ImFontConfig, Ui,
};
use imgui_gfx_renderer::{Renderer, Shaders};

use util::duration_f32;

pub struct ImGuiContext {
    imgui: ImGui,
    renderer: Renderer<gfx_device_gl::Resources>,
    mouse_down: [bool; 5],
}

impl ImGuiContext {
    pub fn new(ctx: &mut Context) -> ImGuiContext {
        let (factory, device, _encoder, _stencil_view, target_view) =
            graphics::get_gfx_objects(ctx);

        let shaders = {
            let version = device.get_info().shading_language;
            if version.is_embedded {
                if version.major >= 3 {
                    Shaders::GlSlEs300
                } else {
                    Shaders::GlSlEs100
                }
            } else if version.major >= 4 {
                Shaders::GlSl400
            } else if version.major >= 3 {
                Shaders::GlSl130
            } else {
                Shaders::GlSl110
            }
        };

        let mut imgui = ImGui::init();
        imgui.set_ini_filename(None);
        imgui.fonts().add_default_font_with_config(
            ImFontConfig::new()
                .oversample_h(1)
                .pixel_snap_h(true)
                .size_pixels(12.0),
        );

        let view = gfx::memory::Typed::new(target_view);  // undocumented type assertion woo
        let renderer = Renderer::init(&mut imgui, factory, shaders, view)
            .unwrap();

        ImGuiContext { imgui, renderer, mouse_down: [false; 5] }
    }

    pub fn frame<'ui, 'a: 'ui>(&'a mut self, ctx: &mut Context) -> ImGuiFrame<'ui> {
        let graphics::Rect { w, h, .. } = graphics::get_screen_coordinates(ctx);
        let fs = imgui::FrameSize { logical_size: (w.into(), h.into()), hidpi_factor: 1.0 };
        ImGuiFrame {
            ui: self.imgui.frame(fs, duration_f32(timer::get_delta(ctx))),
            renderer: &mut self.renderer,
        }
    }
    
    pub fn process_event(&mut self, event: &Event) {
        match event {
            Event::MouseMotion { x, y, .. } => self.imgui.set_mouse_pos(*x as f32, *y as f32),
            _ => {
                if let Some((ix, state)) = match event {
                    Event::MouseButtonDown { mouse_btn, .. } => mb_ix(mouse_btn).map(|ix| (ix, true)),
                    Event::MouseButtonUp { mouse_btn, .. } => mb_ix(mouse_btn).map(|ix| (ix, false)),
                    _ => None,
                } {
                    self.mouse_down[ix] = state;
                    self.imgui.set_mouse_down(self.mouse_down);
                }
            }
        }
    }
}

fn mb_ix(mb: &MouseButton) -> Option<usize> {
    match mb {
        MouseButton::Left => Some(0),
        MouseButton::Right => Some(1),
        MouseButton::Middle => Some(2),
        MouseButton::X1 => Some(3),
        MouseButton::X2 => Some(4),
        MouseButton::Unknown => None,
    }
}

pub struct ImGuiFrame<'ui> {
    pub ui: Ui<'ui>,
    renderer: &'ui mut Renderer<gfx_device_gl::Resources>,
}

impl<'ui> ImGuiFrame<'ui> {
    pub fn render(self, ctx: &mut Context) {
        let (factory, _device, encoder, _stencil_view, _target_view) =
            graphics::get_gfx_objects(ctx);
        self.renderer.render(self.ui, factory, encoder)
            .unwrap();
    }
}