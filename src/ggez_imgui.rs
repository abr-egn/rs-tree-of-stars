use gfx;
use gfx_device_gl;
use ggez::{
    Context,
    graphics, timer,
};

use imgui::{
    self,
    ImGui, ImFontConfig, Ui,
};
use imgui_gfx_renderer::{Renderer, Shaders};

use util::duration_f32;

pub type GgezRenderer = Renderer<gfx_device_gl::Resources>;

pub fn init(ctx: &mut Context) -> (ImGui, GgezRenderer) {
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

    (imgui, renderer)
}

pub fn frame<'ui, 'a: 'ui>(ctx: &mut Context, imgui: &'a mut ImGui) -> Ui<'ui> {
    let graphics::Rect { w, h, .. } = graphics::get_screen_coordinates(ctx);
    let fs = imgui::FrameSize { logical_size: (w.into(), h.into()), hidpi_factor: 1.0 };
    imgui.frame(fs, duration_f32(timer::get_delta(ctx)))
}

pub fn render<'ui>(ctx: &mut Context, renderer: &mut GgezRenderer, ui: Ui<'ui>) {
    let (factory, _device, encoder, _stencil_view, _target_view) =
        graphics::get_gfx_objects(ctx);
    renderer.render(ui, factory, encoder)
        .unwrap();
}