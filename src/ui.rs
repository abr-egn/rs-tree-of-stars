use gfx;
use gfx_device_gl;
use ggez::{
    Context,
    graphics, timer,
};

use imgui::{
    self,
    ImGui, ImFontConfig,
};
use imgui_gfx_renderer::{Renderer, Shaders};

use util::duration_f32;

pub struct UI<R: gfx::Resources> {
    imgui: ImGui,
    renderer: Renderer<R>,
}

pub fn new(ctx: &mut Context) -> UI<gfx_device_gl::Resources> {
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

    UI { imgui, renderer }
}

impl UI<gfx_device_gl::Resources> {
    #[allow(unused)]
    pub fn render<F: FnOnce(&imgui::Ui)>(&mut self, ctx: &mut Context, f: F) {
        let graphics::Rect { w, h, .. } = graphics::get_screen_coordinates(ctx);
        let fs = imgui::FrameSize { logical_size: (w.into(), h.into()), hidpi_factor: 1.0 };
        let ui = self.imgui.frame(fs, duration_f32(timer::get_delta(ctx)));

        f(&ui);

        let (factory, device, encoder, _stencil_view, target_view) =
            graphics::get_gfx_objects(ctx);
        self.renderer.render(ui, factory, encoder)
            .unwrap();
        //encoder.flush(device);
    }
}