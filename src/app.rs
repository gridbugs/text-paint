use crate::palette::Palette;
use gridbugs::{
    chargrid::{self, border::Border, control_flow::*, prelude::*, text},
    grid_2d::Grid,
    line_2d,
    rgb_int::Rgb24,
};
use std::{fmt, path::PathBuf};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum PaletteIndex {
    #[default]
    None,
    Index(usize),
}

impl PaletteIndex {
    fn option(self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Index(i) => Some(i),
        }
    }
}

#[derive(Default)]
struct PerPalette<T> {
    ch: T,
    fg: T,
    bg: T,
}

type PaletteIndices = PerPalette<PaletteIndex>;

type PaletteHover = PerPalette<Option<PaletteIndex>>;

#[derive(Clone, Copy)]
enum Tool {
    Pencil,
    Line,
    Fill,
    Erase,
    Eyedrop,
    Text,
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pencil => "Pencil",
            Self::Line => "Line",
            Self::Fill => "Fill",
            Self::Erase => "Erase",
            Self::Eyedrop => "Eyedrop",
            Self::Text => "Text",
        };
        write!(f, "{}", s)
    }
}

impl Tool {
    fn all() -> Vec<Self> {
        use Tool::*;
        vec![Pencil, Fill, Line, Erase, Eyedrop, Text]
    }
}

struct CanvasState {
    raster: Grid<RenderCell>,
}

impl CanvasState {
    fn new(size: Size) -> Self {
        let cell = RenderCell {
            character: None,
            style: Style::default(),
        };
        Self {
            raster: Grid::new_clone(size, cell),
        }
    }

    fn pencil_coord(&mut self, coord: Coord, cell: RenderCell) {
        if let Some(raster_cell) = self.raster.get_mut(coord) {
            raster_cell.character = cell.character.or(raster_cell.character);
            raster_cell.style.background = cell.style.background.or(raster_cell.style.background);
            raster_cell.style.foreground = cell.style.foreground.or(raster_cell.style.foreground);
            raster_cell.style.bold = cell.style.bold.or(raster_cell.style.bold);
            raster_cell.style.underline = cell.style.bold.or(raster_cell.style.underline);
        }
    }
}

struct AppState {
    palette: Palette,
    palette_indices: PaletteIndices,
    palette_hover: PaletteHover,
    tools: Vec<Tool>,
    tool_index: usize,
    tool_hover: Option<usize>,
    canvas_state: CanvasState,
    canvas_mouse_down_coord: Option<Coord>,
    canvas_hover: Option<Coord>,
}

impl AppState {
    fn new_with_palette(palette: Palette) -> Self {
        Self {
            palette,
            palette_indices: Default::default(),
            palette_hover: Default::default(),
            tools: Tool::all(),
            tool_index: 0,
            tool_hover: None,
            canvas_state: CanvasState::new(Size::new(100, 80)),
            canvas_mouse_down_coord: None,
            canvas_hover: None,
        }
    }

    fn get_ch(&self) -> Option<char> {
        self.palette_indices.ch.option().map(|i| self.palette.ch[i])
    }

    fn get_fg(&self) -> Option<Rgba32> {
        self.palette_indices
            .fg
            .option()
            .map(|i| self.palette.fg[i].to_rgba32(255))
    }

    fn get_bg(&self) -> Option<Rgba32> {
        self.palette_indices
            .bg
            .option()
            .map(|i| self.palette.bg[i].to_rgba32(255))
    }

    fn current_render_cell(&self) -> RenderCell {
        RenderCell {
            character: self.get_ch(),
            style: Style::default()
                .with_foreground_option(self.get_fg())
                .with_background_option(self.get_bg()),
        }
    }

    fn pencil_coord(&mut self, coord: Coord) {
        self.canvas_state
            .pencil_coord(coord, self.current_render_cell());
    }

    fn current_tool(&self) -> Tool {
        self.tools[self.tool_index]
    }

    fn flood_fill(&mut self, coord: Coord) {
        use gridbugs::direction::CardinalDirection;
        use std::collections::{HashSet, VecDeque};
        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();
        let initial_cell = self.canvas_state.raster.get_checked(coord);
        queue.push_front(coord);
        seen.insert(coord);
        while let Some(coord) = queue.pop_back() {
            for d in CardinalDirection::all() {
                let nei_coord = coord + d.coord();
                if !seen.contains(&nei_coord) {
                    if let Some(nei_cell) = self.canvas_state.raster.get(nei_coord) {
                        if nei_cell == initial_cell {
                            seen.insert(nei_coord);
                            queue.push_front(nei_coord);
                        }
                    }
                }
            }
        }
        for coord in seen {
            self.pencil_coord(coord);
        }
    }
}

struct PaletteComponent {
    ch_label: text::StyledString,
    fg_label: text::StyledString,
    bg_label: text::StyledString,
}

impl PaletteComponent {
    fn new() -> Self {
        Self {
            ch_label: text::StyledString::plain_text("ch|".to_string()),
            fg_label: text::StyledString::plain_text("fg|".to_string()),
            bg_label: text::StyledString::plain_text("bg|".to_string()),
        }
    }
    fn palette_x_offset(&self) -> i32 {
        self.ch_label.string.len() as i32
    }
    fn preview_offset(&self) -> i32 {
        2
    }
}

impl Component for PaletteComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        {
            fb.set_cell_relative_to_ctx(ctx, Coord::new(0, 1), 0, state.current_render_cell());
        }
        let ctx = ctx.add_x(self.preview_offset());
        self.ch_label.render(&(), ctx, fb);
        self.fg_label.render(&(), ctx.add_y(1), fb);
        self.bg_label.render(&(), ctx.add_y(2), fb);
        let ctx = ctx.add_x(self.palette_x_offset());
        let hover_style = Style::plain_text().with_background(Rgba32::new_grey(127));
        let select_style = Style::plain_text()
            .with_foreground(Rgba32::new_grey(0))
            .with_background(Rgba32::new_grey(255));
        {
            let style = if state.palette_indices.ch == PaletteIndex::None {
                select_style
            } else if state.palette_hover.ch == Some(PaletteIndex::None) {
                hover_style
            } else {
                Style::plain_text()
            };
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(0, 0),
                0,
                RenderCell {
                    character: Some('x'),
                    style,
                },
            );
        }
        {
            let style = if state.palette_indices.fg == PaletteIndex::None {
                select_style
            } else if state.palette_hover.fg == Some(PaletteIndex::None) {
                hover_style
            } else {
                Style::plain_text()
            };
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(0, 1),
                0,
                RenderCell {
                    character: Some('x'),
                    style,
                },
            );
        }
        {
            let style = if state.palette_indices.bg == PaletteIndex::None {
                select_style
            } else if state.palette_hover.bg == Some(PaletteIndex::None) {
                hover_style
            } else {
                Style::plain_text()
            };
            fb.set_cell_relative_to_ctx(
                ctx,
                Coord::new(0, 2),
                0,
                RenderCell {
                    character: Some('x'),
                    style,
                },
            );
        }
        let ctx = ctx.add_x(1);
        for (i, &ch) in state.palette.ch.iter().enumerate() {
            let style = if PaletteIndex::Index(i) == state.palette_indices.ch {
                select_style
            } else if Some(PaletteIndex::Index(i)) == state.palette_hover.ch {
                hover_style
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
        fn black_foreground(Rgb24 { r, g, b }: Rgb24) -> bool {
            r as u16 + g as u16 + b as u16 > 320
        }
        for (i, &fg) in state.palette.fg.iter().enumerate() {
            let character = if PaletteIndex::Index(i) == state.palette_indices.fg {
                Some('*')
            } else if Some(PaletteIndex::Index(i)) == state.palette_hover.fg {
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
            let character = if PaletteIndex::Index(i) == state.palette_indices.bg {
                Some('*')
            } else if Some(PaletteIndex::Index(i)) == state.palette_hover.bg {
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
            let ctx = ctx.add_x(self.preview_offset());
            let ctx = ctx.add_x(self.palette_x_offset());
            let ch_bb = ctx
                .bounding_box
                .set_height(1)
                .set_width(state.palette.ch.len() as u32);
            let fg_bb = ctx
                .bounding_box
                .add_y(1)
                .set_height(1)
                .set_width(state.palette.fg.len() as u32);
            let bg_bb = ctx
                .bounding_box
                .add_y(2)
                .set_height(1)
                .set_width(state.palette.bg.len() as u32);
            fn coord_to_index(c: Coord) -> PaletteIndex {
                if c.x == 0 {
                    PaletteIndex::None
                } else {
                    PaletteIndex::Index(c.x as usize - 1)
                }
            }
            match mouse_input {
                MouseInput::MouseMove { coord, .. } => {
                    state.palette_hover.ch =
                        ch_bb.coord_absolute_to_relative(coord).map(coord_to_index);
                    state.palette_hover.fg =
                        fg_bb.coord_absolute_to_relative(coord).map(coord_to_index);
                    state.palette_hover.bg =
                        bg_bb.coord_absolute_to_relative(coord).map(coord_to_index);
                }
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord,
                } => {
                    if let Some(coord) = ch_bb.coord_absolute_to_relative(coord) {
                        state.palette_indices.ch = coord_to_index(coord);
                    }
                    if let Some(coord) = fg_bb.coord_absolute_to_relative(coord) {
                        state.palette_indices.fg = coord_to_index(coord);
                    }
                    if let Some(coord) = bg_bb.coord_absolute_to_relative(coord) {
                        state.palette_indices.bg = coord_to_index(coord);
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
            } else if Some(i) == state.tool_hover {
                let asterisk = text::StyledString {
                    string: "*".to_string(),
                    style: Style::plain_text().with_foreground(Rgba32::new_grey(127)),
                };

                text::Text::new(vec![
                    asterisk.clone(),
                    text::StyledString::plain_text(format!("{}", tool)),
                    asterisk,
                ])
                .render(&(), ctx, fb);
            } else {
                text::StyledString::plain_text(format!(" {}", tool)).render(&(), ctx, fb);
            }
        }
    }
    fn update(&mut self, state: &mut Self::State, ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            use input::MouseButton;
            match mouse_input {
                MouseInput::MouseMove { coord, .. } => {
                    state.tool_hover = ctx
                        .bounding_box
                        .coord_absolute_to_relative(coord)
                        .map(|c| c.y as usize);
                }
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord,
                } => {
                    if let Some(coord) = ctx.bounding_box.coord_absolute_to_relative(coord) {
                        state.tool_index = coord.y as usize;
                    }
                }
                _ => (),
            }
        }
    }
    fn size(&self, state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(10, state.tools.len() as u32)
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
            match state.current_tool() {
                Tool::Pencil => match mouse_input {
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
                            if let Some(prev_coord) = state.canvas_mouse_down_coord {
                                for coord in line_2d::coords_between(prev_coord, coord) {
                                    state.pencil_coord(coord);
                                }
                            } else {
                                state.pencil_coord(coord);
                            }
                            state.canvas_mouse_down_coord = Some(coord);
                        }
                    }
                    MouseInput::MouseRelease { .. } => state.canvas_mouse_down_coord = None,
                    _ => (),
                },
                Tool::Fill => match mouse_input {
                    MouseInput::MousePress {
                        button: MouseButton::Left,
                        coord,
                    } => {
                        if let Some(coord) = ctx.bounding_box.coord_absolute_to_relative(coord) {
                            state.flood_fill(coord);
                        }
                    }
                    _ => (),
                },

                other_tool => eprintln!("{} is unimplemented", other_tool),
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
        let palette = Self::border(PaletteComponent::new(), "Palette");
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
        .exit_on_close()
}
