use crate::palette::Palette;
use gridbugs::chargrid::{self, border::Border, control_flow::*, prelude::*, text};
use std::path::PathBuf;

struct AppState {
    palette: Palette,
}

struct PaletteComponent;

impl Component for PaletteComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        text::StyledString::plain_text("ch: ".to_string()).render(&(), ctx, fb);
        text::StyledString::plain_text("fg: ".to_string()).render(&(), ctx.add_y(1), fb);
        text::StyledString::plain_text("bg: ".to_string()).render(&(), ctx.add_y(2), fb);
        let ctx = ctx.add_x(4);
        for (i, &ch) in state.palette.ch.iter().enumerate() {
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(i as i32, 0),
                0,
                RenderCell {
                    character: Some(ch),
                    style: Style::plain_text(),
                },
            );
        }
        for (i, &fg) in state.palette.fg.iter().enumerate() {
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(i as i32, 1),
                0,
                RenderCell {
                    character: None,
                    style: Style::default().with_background(fg.to_rgba32(255)),
                },
            );
        }
        for (i, &bg) in state.palette.bg.iter().enumerate() {
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(i as i32, 2),
                0,
                RenderCell {
                    character: None,
                    style: Style::default().with_background(bg.to_rgba32(255)),
                },
            );
        }
    }
    fn update(&mut self, _state: &mut Self::State, _ctx: Ctx, _event: Event) -> Self::Output {}
    fn size(&self, _state: &Self::State, ctx: Ctx) -> Size {
        ctx.bounding_box.size().set_height(3)
    }
}

struct ToolsComponent;

impl Component for ToolsComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, _state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        text::StyledString::plain_text("Pencil".to_string()).render(&(), ctx, fb);
        text::StyledString::plain_text("Line".to_string()).render(&(), ctx.add_y(2), fb);
        text::StyledString::plain_text("Fill".to_string()).render(&(), ctx.add_y(1), fb);
    }
    fn update(&mut self, _state: &mut Self::State, _ctx: Ctx, _event: Event) -> Self::Output {}
    fn size(&self, _state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(8, 5)
    }
}

struct CanvasComponent;

impl Component for CanvasComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, _state: &Self::State, _ctx: Ctx, _fb: &mut FrameBuffer) {}
    fn update(&mut self, _state: &mut Self::State, _ctx: Ctx, _event: Event) -> Self::Output {}
    fn size(&self, _state: &Self::State, ctx: Ctx) -> Size {
        ctx.bounding_box.size()
    }
}

struct GuiComponent {
    palette: Border<PaletteComponent>,
    tools: Border<ToolsComponent>,
    canvas: Border<CanvasComponent>,
}

impl GuiComponent {
    fn border<C: Component>(component: C, title: &str) -> Border<C> {
        use chargrid::border::*;
        let colour = Rgba32::new_grey(127);
        Border {
            component,
            style: BorderStyle {
                title: Some(title.to_string()),
                chars: BorderChars::double_line_light().with_title_separators('╡', '╞'),
                foreground: colour,
                title_style: Style::plain_text().with_foreground(colour),
                padding: BorderPadding::all(0),
                ..Default::default()
            },
        }
    }
    fn new() -> Self {
        let palette = Self::border(PaletteComponent, "Palette");
        let tools = Self::border(ToolsComponent, "Tools");
        let canvas = Self::border(CanvasComponent, "Canvas");
        Self {
            palette,
            tools,
            canvas,
        }
    }
}

impl Component for GuiComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        let palette_size = self.palette.size(state, ctx);
        let tools_size = self.tools.size(state, ctx);
        let palette_ctx =
            ctx.add_y(ctx.bounding_box.size().height() as i32 - palette_size.height() as i32);
        let height_above_palette =
            (ctx.bounding_box.size().height() as i32 - palette_size.height() as i32) as u32;
        let tools_ctx = ctx.set_height(height_above_palette);
        let canvas_ctx = ctx
            .set_height(height_above_palette)
            .add_x(tools_size.width() as i32);
        self.palette.render(state, palette_ctx, fb);
        self.tools.render(state, tools_ctx, fb);
        self.canvas.render(state, canvas_ctx, fb);
    }
    fn update(&mut self, _state: &mut Self::State, _ctx: Ctx, _event: Event) -> Self::Output {}
    fn size(&self, _state: &Self::State, ctx: Ctx) -> Size {
        ctx.bounding_box.size()
    }
}

pub fn app(palette_path: PathBuf) -> App {
    let palette = Palette::load(palette_path).unwrap();
    let app_state = AppState { palette };
    cf(GuiComponent::new())
        .ignore_output()
        .with_state(app_state)
        .catch_escape()
        .map(|res| match res {
            Err(Escape) => app::Exit,
            Ok(output) => output,
        })
        .clear_each_frame()
}
