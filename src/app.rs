use crate::palette::Palette;
use gridbugs::{
    chargrid::{self, border::Border, control_flow::*, prelude::*, text},
    grid_2d::Grid,
};
use std::{fmt, path::PathBuf};

#[derive(Default)]
struct PaletteState {
    ch_index: usize,
    fg_index: usize,
    bg_index: usize,
}

#[derive(Default)]
struct PaletteHover {
    ch_index: Option<usize>,
    fg_index: Option<usize>,
    bg_index: Option<usize>,
}

enum Tool {
    Pencil,
    Line,
    Fill,
    Erase,
    Eyedrop,
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pencil => "Pencil",
            Self::Line => "Line",
            Self::Fill => "Fill",
            Self::Erase => "Erase",
            Self::Eyedrop => "Eyedrop",
        };
        write!(f, "{}", s)
    }
}

impl Tool {
    fn all() -> Vec<Self> {
        use Tool::*;
        vec![Pencil, Line, Fill, Erase, Eyedrop]
    }
}

struct CanvasState {
    raster: Grid<RenderCell>,
}

impl CanvasState {
    fn new(size: Size) -> Self {
        let cell = RenderCell {
            character: None,
            style: Style::plain_text(),
        };
        Self {
            raster: Grid::new_clone(size, cell),
        }
    }

    fn pencil_coord(&mut self, coord: Coord, cell: RenderCell) {
        if let Some(raster_cell) = self.raster.get_mut(coord) {
            *raster_cell = cell;
        }
    }
}

struct AppState {
    palette: Palette,
    palette_state: PaletteState,
    palette_hover: PaletteHover,
    tools: Vec<Tool>,
    tool_index: usize,
    canvas_state: CanvasState,
    canvas_mouse_down_coord: Option<Coord>,
    canvas_hover: Option<Coord>,
}

impl AppState {
    fn new_with_palette(palette: Palette) -> Self {
        Self {
            palette,
            palette_state: Default::default(),
            palette_hover: Default::default(),
            tools: Tool::all(),
            tool_index: 0,
            canvas_state: CanvasState::new(Size::new(45, 30)),
            canvas_mouse_down_coord: None,
            canvas_hover: None,
        }
    }

    fn current_render_cell(&self) -> RenderCell {
        RenderCell {
            character: Some(self.palette.ch[self.palette_state.ch_index]),
            style: Style::default()
                .with_foreground(self.palette.fg[self.palette_state.fg_index].to_rgba32(255))
                .with_background(self.palette.bg[self.palette_state.bg_index].to_rgba32(255)),
        }
    }

    fn pencil_coord(&mut self, coord: Coord) {
        self.canvas_state
            .pencil_coord(coord, self.current_render_cell());
    }
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
            let style = if i == state.palette_state.ch_index {
                Style::plain_text()
                    .with_foreground(Rgba32::new_grey(0))
                    .with_background(Rgba32::new_grey(255))
            } else if Some(i) == state.palette_hover.ch_index {
                Style::plain_text().with_background(Rgba32::new_grey(127))
            } else {
                Style::plain_text()
            };
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(i as i32, 0),
                0,
                RenderCell {
                    character: Some(ch),
                    style,
                },
            );
        }
        use gridbugs::rgb_int::Rgb24;
        fn black_foreground(Rgb24 { r, g, b }: Rgb24) -> bool {
            r as u16 + g as u16 + b as u16 > 320
        }
        for (i, &fg) in state.palette.fg.iter().enumerate() {
            let character = if i == state.palette_state.fg_index {
                Some('*')
            } else if Some(i) == state.palette_hover.fg_index {
                Some('+')
            } else {
                None
            };
            let foreground = if black_foreground(fg) {
                Rgba32::new_grey(0)
            } else {
                Rgba32::new_grey(255)
            };
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(i as i32, 1),
                0,
                RenderCell {
                    character,
                    style: Style::default()
                        .with_background(fg.to_rgba32(255))
                        .with_foreground(foreground),
                },
            );
        }
        for (i, &bg) in state.palette.bg.iter().enumerate() {
            let character = if i == state.palette_state.bg_index {
                Some('*')
            } else if Some(i) == state.palette_hover.bg_index {
                Some('+')
            } else {
                None
            };
            let foreground = if black_foreground(bg) {
                Rgba32::new_grey(0)
            } else {
                Rgba32::new_grey(255)
            };
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(i as i32, 2),
                0,
                RenderCell {
                    character,
                    style: Style::default()
                        .with_background(bg.to_rgba32(255))
                        .with_foreground(foreground),
                },
            );
        }
    }
    fn update(&mut self, state: &mut Self::State, ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            use input::MouseButton;
            let ch_bb = ctx
                .bounding_box
                .add_offset(Coord::new(4, 0))
                .set_height(1)
                .set_width(state.palette.ch.len() as u32);
            let fg_bb = ctx
                .bounding_box
                .add_offset(Coord::new(4, 1))
                .set_height(1)
                .set_width(state.palette.fg.len() as u32);
            let bg_bb = ctx
                .bounding_box
                .add_offset(Coord::new(4, 2))
                .set_height(1)
                .set_width(state.palette.bg.len() as u32);
            match mouse_input {
                MouseInput::MouseMove { coord, .. } => {
                    state.palette_hover.ch_index = ch_bb
                        .coord_absolute_to_relative(coord)
                        .map(|c| c.x as usize);
                    state.palette_hover.fg_index = fg_bb
                        .coord_absolute_to_relative(coord)
                        .map(|c| c.x as usize);
                    state.palette_hover.bg_index = bg_bb
                        .coord_absolute_to_relative(coord)
                        .map(|c| c.x as usize);
                }
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord,
                } => {
                    if let Some(coord) = ch_bb.coord_absolute_to_relative(coord) {
                        state.palette_state.ch_index = coord.x as usize;
                    }
                    if let Some(coord) = fg_bb.coord_absolute_to_relative(coord) {
                        state.palette_state.fg_index = coord.x as usize;
                    }
                    if let Some(coord) = bg_bb.coord_absolute_to_relative(coord) {
                        state.palette_state.bg_index = coord.x as usize;
                    }
                }
                _ => (),
            }
        }
    }
    fn size(&self, _state: &Self::State, ctx: Ctx) -> Size {
        ctx.bounding_box.size().set_height(3)
    }
}

struct ToolsComponent;

impl Component for ToolsComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        for (i, tool) in state.tools.iter().enumerate() {
            let ctx = ctx.add_y(i as i32);
            if i == state.tool_index {
                text::StyledString::plain_text(format!("*{}*", tool)).render(&(), ctx, fb);
            } else {
                text::StyledString::plain_text(format!(" {}", tool)).render(&(), ctx, fb);
            }
        }
    }
    fn update(&mut self, _state: &mut Self::State, _ctx: Ctx, _event: Event) -> Self::Output {}
    fn size(&self, _state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(10, 5)
    }
}

struct CanvasComponent;

impl Component for CanvasComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        for (coord, &cell) in state.canvas_state.raster.enumerate() {
            let mut cell = cell;
            if Some(coord) == state.canvas_hover {
                cell.style.background = if let Some(background) = cell.background() {
                    Some(background.saturating_scalar_mul_div(4, 3))
                } else {
                    Some(Rgba32::new_grey(127))
                };
            }
            fb.set_cell_relative_to_ctx(ctx, coord, 0, cell);
        }
    }
    fn update(&mut self, state: &mut Self::State, ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            use input::MouseButton;
            state.canvas_hover = ctx
                .bounding_box
                .coord_absolute_to_relative(mouse_input.coord());
            match mouse_input {
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord,
                } => {
                    if let Some(coord) = ctx.bounding_box.coord_absolute_to_relative(coord) {
                        state.canvas_mouse_down_coord = Some(coord);
                        state.pencil_coord(coord);
                    }
                }
                MouseInput::MouseMove {
                    button: Some(MouseButton::Left),
                    coord,
                } => {
                    if let Some(coord) = ctx.bounding_box.coord_absolute_to_relative(coord) {
                        state.pencil_coord(coord);
                    }
                }
                MouseInput::MouseRelease { .. } => state.canvas_mouse_down_coord = None,

                _ => (),
            }
        }
    }
    fn size(&self, state: &Self::State, ctx: Ctx) -> Size {
        state
            .canvas_state
            .raster
            .size()
            .pairwise_min(ctx.bounding_box.size())
    }
}

struct GuiComponent {
    palette: Border<PaletteComponent>,
    tools: Border<ToolsComponent>,
    canvas: Border<CanvasComponent>,
}

struct GuiChildCtxs<'a> {
    palette: Ctx<'a>,
    tools: Ctx<'a>,
    canvas: Ctx<'a>,
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

    fn child_ctxs<'a>(&self, state: &AppState, ctx: Ctx<'a>) -> GuiChildCtxs<'a> {
        let palette_size = self.palette.size(state, ctx);
        let tools_size = self.tools.size(state, ctx);
        let palette =
            ctx.add_y(ctx.bounding_box.size().height() as i32 - palette_size.height() as i32);
        let height_above_palette =
            (ctx.bounding_box.size().height() as i32 - palette_size.height() as i32) as u32;
        let tools = ctx.set_size(tools_size);
        let canvas = ctx
            .set_height(height_above_palette)
            .add_x(tools_size.width() as i32);
        GuiChildCtxs {
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
        let ctxs = self.child_ctxs(state, ctx);
        self.palette.render(state, ctxs.palette, fb);
        self.tools.render(state, ctxs.tools, fb);
        self.canvas.render(state, ctxs.canvas, fb);
    }
    fn update(&mut self, state: &mut Self::State, ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            let ctxs = self.child_ctxs(state, ctx);
            if ctxs
                .palette
                .bounding_box
                .contains_coord(mouse_input.coord())
            {
                self.palette.update(state, ctxs.palette, event)
            }
            if ctxs.tools.bounding_box.contains_coord(mouse_input.coord()) {
                self.tools.update(state, ctxs.tools, event)
            }
            if ctxs.canvas.bounding_box.contains_coord(mouse_input.coord()) {
                self.canvas.update(state, ctxs.canvas, event)
            }
        }
    }
    fn size(&self, _state: &Self::State, ctx: Ctx) -> Size {
        ctx.bounding_box.size()
    }
}

pub fn app(palette_path: PathBuf) -> App {
    let palette = Palette::load(palette_path).unwrap();
    let app_state = AppState::new_with_palette(palette);
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
