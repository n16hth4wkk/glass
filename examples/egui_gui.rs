use egui::FullOutput;
use egui_demo_lib::DemoWindows;
use egui_wgpu::renderer::ScreenDescriptor;
use egui_winit::EventResponse;
use glass::{
    window::GlassWindow, Glass, GlassApp, GlassConfig, GlassContext, GlassError, RenderData,
};
use wgpu::{CommandEncoder, TextureView};
use winit::{
    event::Event,
    event_loop::{EventLoop, EventLoopWindowTarget},
};

fn main() -> Result<(), GlassError> {
    Glass::new(GuiApp::default(), GlassConfig::default()).run()
}

impl GlassApp for GuiApp {
    fn start(&mut self, event_loop: &EventLoop<()>, context: &mut GlassContext) {
        initialize_gui_app(self, context, event_loop);
    }

    fn input(
        &mut self,
        context: &mut GlassContext,
        _event_loop: &EventLoopWindowTarget<()>,
        event: &Event<()>,
    ) {
        update_egui_with_winit_event(self, context, event);
    }

    fn render(&mut self, context: &GlassContext, render_data: RenderData) {
        render(self, context, render_data);
    }
}

#[derive(Default)]
struct GuiApp {
    gui: Option<GuiState>,
}

impl GuiApp {
    fn gui(&mut self) -> &mut GuiState {
        self.gui.as_mut().unwrap()
    }
}

struct GuiState {
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    repaint: bool,
    ui_app: DemoWindows,
}

fn initialize_gui_app(
    app: &mut GuiApp,
    context: &mut GlassContext,
    event_loop: &EventLoopWindowTarget<()>,
) {
    let mut egui_winit = egui_winit::State::new(event_loop);
    let renderer =
        egui_wgpu::Renderer::new(context.device(), GlassWindow::surface_format(), None, 1);

    egui_winit.set_max_texture_side(context.device().limits().max_texture_dimension_2d as usize);
    let pixels_per_point = context.primary_render_window().window().scale_factor() as f32;
    egui_winit.set_pixels_per_point(pixels_per_point);
    app.gui = Some(GuiState {
        egui_ctx: egui::Context::default(),
        egui_winit,
        renderer,
        repaint: false,
        ui_app: egui_demo_lib::DemoWindows::default(),
    });
}

fn update_egui_with_winit_event(app: &mut GuiApp, _context: &mut GlassContext, event: &Event<()>) {
    match event {
        Event::WindowEvent {
            event, ..
        } => {
            let gui = app.gui();
            let EventResponse {
                consumed,
                repaint,
            } = gui.egui_winit.on_event(&gui.egui_ctx, event);
            gui.repaint = repaint;
            // Skip input if event was consumed by egui
            if consumed {
                return;
            }
        }
        _ => {}
    }
}

fn render(app: &mut GuiApp, context: &GlassContext, render_data: RenderData) {
    let RenderData {
        encoder,
        frame,
        ..
    } = render_data;
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    render_egui(app, context, encoder, &view);
}

fn render_egui(
    app: &mut GuiApp,
    context: &GlassContext,
    encoder: &mut CommandEncoder,
    view: &TextureView,
) {
    let window = context.primary_render_window();
    let GuiState {
        egui_ctx,
        renderer,
        egui_winit,
        ui_app,
        ..
    } = app.gui();
    let raw_input = egui_winit.take_egui_input(window.window());
    let FullOutput {
        shapes,
        textures_delta,
        ..
    } = egui_ctx.run(raw_input, |egui_ctx| {
        // Ui content
        ui_app.ui(egui_ctx);
    });
    // creates triangles to paint
    let clipped_primitives = egui_ctx.tessellate(shapes);

    let size = window.surface_size();
    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: size,
        pixels_per_point: window.window().scale_factor() as f32,
    };

    // Upload all resources for the GPU.
    let user_cmd_bufs = {
        for (id, image_delta) in &textures_delta.set {
            renderer.update_texture(context.device(), context.queue(), *id, image_delta);
        }

        // Update buffers
        renderer.update_buffers(
            context.device(),
            context.queue(),
            encoder,
            &clipped_primitives,
            &screen_descriptor,
        )
    };

    // Render
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        // Here you would render your scene
        // Render Egui
        renderer.render(&mut render_pass, &*clipped_primitives, &screen_descriptor);
    }

    for id in &textures_delta.free {
        renderer.free_texture(id);
    }

    // Submit user cmd buffers
    context.queue().submit(user_cmd_bufs.into_iter());
}
