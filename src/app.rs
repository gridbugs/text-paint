use crate::palette::Palette;
use gridbugs::chargrid::{self, border::Border, control_flow::*, prelude::*};
use std::path::PathBuf;

struct AppState {
    palette: Palette,
}

struct PaletteComponent;

impl Component for PaletteComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        chargrid::text::StyledString::plain_text("ch: ".to_string()).render(&(), ctx, fb);
        chargrid::text::StyledString::plain_text("fg: ".to_string()).render(&(), ctx.add_y(1), fb);
        chargrid::text::StyledString::plain_text("bg: ".to_string()).render(&(), ctx.add_y(2), fb);
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

struct GuiComponent {
    palette: Border<PaletteComponent>,
}

impl GuiComponent {
    fn new() -> Self {
        use chargrid::border::*;
        let palette = Border {
            component: PaletteComponent,
            style: BorderStyle {
                title: Some("Palette".to_string()),
                chars: BorderChars::double_line_light().with_title_separators('╡', '╞'),
                ..Default::default()
            },
        };
        Self { palette }
    }
}

impl Component for GuiComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        let palette_ctx = ctx.add_y(
            ctx.bounding_box.size().height() as i32 - self.palette.size(state, ctx).height() as i32,
        );
        self.palette.render(state, palette_ctx, fb);
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
