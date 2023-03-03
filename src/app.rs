use crate::palette::Palette;
use gridbugs::{
    chargrid::{self, border::Border, control_flow::*, prelude::*, text, text_field::TextField},
    grid_2d::Grid,
    line_2d,
    rgb_int::Rgb24,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    iter,
    path::{Path, PathBuf},
};

#[derive(Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Default, Serialize, Deserialize)]
struct PerPalette<T> {
    ch: T,
    fg: T,
    bg: T,
}

type PaletteIndices = PerPalette<Option<PaletteIndex>>;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        vec![Pencil, Fill, Line, Erase, Eyedrop]
    }

    fn new_event(self, coord: Coord) -> Option<DrawingEvent> {
        match self {
            Self::Pencil => Some(DrawingEvent::pencil(coord)),
            Self::Fill => Some(DrawingEvent::flood_fill(coord)),
            Self::Line => Some(DrawingEvent::line(coord)),
            Self::Erase => Some(DrawingEvent::erase(coord)),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PencilEvent {
    coords: HashMap<Coord, u32>,
    last_coord: Coord,
}

impl PencilEvent {
    fn mouse_press(coord: Coord) -> Self {
        Self {
            coords: iter::once((coord, 1)).collect(),
            last_coord: coord,
        }
    }
    fn mouse_move(&mut self, coord: Coord) {
        if coord != self.last_coord {
            let iter =
                line_2d::LineSegment::new(self.last_coord, coord).config_iter(line_2d::Config {
                    exclude_start: true,
                    exclude_end: false,
                });
            for coord in iter {
                *self.coords.entry(coord).or_insert(0) += 1;
            }
            self.last_coord = coord;
        }
    }
    fn commit(&self, render_cell: RenderCell, raster: &mut Raster) {
        for (&coord, &count) in self.coords.iter() {
            for _ in 0..count {
                raster.set_coord(coord, render_cell);
            }
        }
    }
    fn preview(&self, raster: &Raster, render_cell: RenderCell, ctx: Ctx, fb: &mut FrameBuffer) {
        for (&coord, &count) in self.coords.iter() {
            // chargrid's alpha compositing doesn't blend foreground colours so fake it here
            if let Some(&stacked_render_cell) = raster.grid.get(coord) {
                let mut stacked_render_cell = stacked_render_cell;
                for _ in 0..count {
                    stacked_render_cell =
                        Raster::stack_render_cells(stacked_render_cell, render_cell);
                }
                fb.set_cell_relative_to_ctx(ctx, coord, 0, stacked_render_cell);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct FillEvent {
    start: Coord,
}

impl FillEvent {
    fn mouse_press(coord: Coord) -> Self {
        Self { start: coord }
    }
    fn mouse_move(&mut self, coord: Coord) {
        self.start = coord;
    }
    fn commit(&self, render_cell: RenderCell, raster: &mut Raster) {
        for coord in raster.flood_fill(self.start) {
            raster.set_coord(coord, render_cell);
        }
    }
    fn preview(&self, raster: &Raster, render_cell: RenderCell, ctx: Ctx, fb: &mut FrameBuffer) {
        for coord in raster.flood_fill(self.start) {
            if let Some(&current_cell) = raster.grid.get(coord) {
                let stacked_render_cell = Raster::stack_render_cells(current_cell, render_cell);
                fb.set_cell_relative_to_ctx(ctx, coord, 0, stacked_render_cell);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct LineEvent {
    start: Coord,
    end: Coord,
}

impl LineEvent {
    fn mouse_press(coord: Coord) -> Self {
        Self {
            start: coord,
            end: coord,
        }
    }
    fn mouse_move(&mut self, coord: Coord) {
        self.end = coord;
    }
    fn commit(&self, render_cell: RenderCell, raster: &mut Raster) {
        for coord in line_2d::coords_between(self.start, self.end) {
            raster.set_coord(coord, render_cell);
        }
    }
    fn preview(&self, raster: &Raster, render_cell: RenderCell, ctx: Ctx, fb: &mut FrameBuffer) {
        for coord in line_2d::coords_between(self.start, self.end) {
            if let Some(&current_cell) = raster.grid.get(coord) {
                let stacked_render_cell = Raster::stack_render_cells(current_cell, render_cell);
                fb.set_cell_relative_to_ctx(ctx, coord, 0, stacked_render_cell);
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct EraseEvent {
    coords: HashSet<Coord>,
    last_coord: Coord,
}

impl EraseEvent {
    fn mouse_press(coord: Coord) -> Self {
        Self {
            coords: iter::once(coord).collect(),
            last_coord: coord,
        }
    }
    fn mouse_move(&mut self, coord: Coord) {
        for coord in line_2d::coords_between(self.last_coord, coord) {
            self.coords.insert(coord);
        }
        self.last_coord = coord;
    }
    fn commit(&self, raster: &mut Raster) {
        for &coord in self.coords.iter() {
            raster.clear_coord(coord);
        }
    }
    fn preview(&self, ctx: Ctx, fb: &mut FrameBuffer) {
        let blank_render_cell = RenderCell {
            character: Some('█'),
            style: Style {
                foreground: Some(Rgba32::new_grey(0)),
                ..Default::default()
            },
        };
        for &coord in self.coords.iter() {
            fb.set_cell_relative_to_ctx(ctx, coord, 0, blank_render_cell);
        }
    }
}

#[derive(Serialize, Deserialize)]
enum DrawingEvent {
    Pencil(PencilEvent),
    Fill(FillEvent),
    Line(LineEvent),
    Erase(EraseEvent),
}

impl DrawingEvent {
    fn pencil(coord: Coord) -> Self {
        Self::Pencil(PencilEvent::mouse_press(coord))
    }
    fn flood_fill(coord: Coord) -> Self {
        Self::Fill(FillEvent::mouse_press(coord))
    }
    fn line(coord: Coord) -> Self {
        Self::Line(LineEvent::mouse_press(coord))
    }
    fn erase(coord: Coord) -> Self {
        Self::Erase(EraseEvent::mouse_press(coord))
    }
    fn mouse_move(&mut self, coord: Coord) {
        match self {
            Self::Pencil(pencil) => pencil.mouse_move(coord),
            Self::Fill(flood_fill) => flood_fill.mouse_move(coord),
            Self::Line(line) => line.mouse_move(coord),
            Self::Erase(erase) => erase.mouse_move(coord),
        }
    }
    fn commit(&self, render_cell: RenderCell, raster: &mut Raster) {
        match self {
            Self::Pencil(pencil) => pencil.commit(render_cell, raster),
            Self::Fill(flood_fill) => flood_fill.commit(render_cell, raster),
            Self::Line(line) => line.commit(render_cell, raster),
            Self::Erase(erase) => erase.commit(raster),
        }
    }
    fn preview(&self, raster: &Raster, render_cell: RenderCell, ctx: Ctx, fb: &mut FrameBuffer) {
        match self {
            Self::Pencil(pencil) => pencil.preview(raster, render_cell, ctx, fb),
            Self::Fill(flood_fill) => flood_fill.preview(raster, render_cell, ctx, fb),
            Self::Line(line) => line.preview(raster, render_cell, ctx, fb),
            Self::Erase(erase) => erase.preview(ctx, fb),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Raster {
    grid: Grid<RenderCell>,
}

impl Raster {
    fn new(size: Size) -> Self {
        let cell = RenderCell {
            character: None,
            style: Style::default().with_background(Rgba32::new_grey(0)),
        };
        Self {
            grid: Grid::new_clone(size, cell),
        }
    }

    fn stack_render_cells(bottom: RenderCell, top: RenderCell) -> RenderCell {
        fn blend(a: Option<Rgba32>, b: Option<Rgba32>) -> Option<Rgba32> {
            match (a, b) {
                (None, None) => None,
                (Some(x), None) | (None, Some(x)) => Some(x),
                (Some(a), Some(b)) => Some(a.alpha_composite(b)),
            }
        }
        let mut ret = bottom;
        ret.character = top.character.or(bottom.character);
        ret.style.background = blend(top.style.background, bottom.style.background);
        // blend the foreground with the background if there is currently no character present
        let bottom_foreground = if bottom.character.is_none() {
            bottom.style.background
        } else {
            bottom.style.foreground
        };
        ret.style.foreground = blend(top.style.foreground, bottom_foreground);
        ret.style.bold = top.style.bold.or(bottom.style.bold);
        ret.style.underline = top.style.bold.or(bottom.style.underline);
        ret
    }

    fn set_coord(&mut self, coord: Coord, cell: RenderCell) {
        if let Some(raster_cell) = self.grid.get_mut(coord) {
            *raster_cell = Self::stack_render_cells(*raster_cell, cell);
        }
    }

    fn clear_coord(&mut self, coord: Coord) {
        if let Some(raster_cell) = self.grid.get_mut(coord) {
            *raster_cell = RenderCell {
                character: None,
                style: Style::default().with_background(Rgba32::new_grey(0)),
            };
        }
    }
    fn flood_fill(&self, coord: Coord) -> HashSet<Coord> {
        use gridbugs::direction::CardinalDirection;
        use std::collections::VecDeque;
        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();
        let initial_cell = self.grid.get_checked(coord);
        queue.push_front(coord);
        seen.insert(coord);
        while let Some(coord) = queue.pop_back() {
            for d in CardinalDirection::all() {
                let nei_coord = coord + d.coord();
                if !seen.contains(&nei_coord) {
                    if let Some(nei_cell) = self.grid.get(nei_coord) {
                        if nei_cell == initial_cell {
                            seen.insert(nei_coord);
                            queue.push_front(nei_coord);
                        }
                    }
                }
            }
        }
        seen
    }

    fn commit_event(&mut self, event: &DrawingEventWithRenderCell) {
        event.drawing_event.commit(event.render_cell, self);
    }
}

#[derive(Serialize, Deserialize)]
struct DrawingEventWithRenderCell {
    drawing_event: DrawingEvent,
    render_cell: RenderCell,
}

#[derive(Serialize, Deserialize)]
struct UndoBuffer {
    initial: Raster,
    events: Vec<DrawingEventWithRenderCell>,
    redo_buffer: Vec<DrawingEventWithRenderCell>,
}

impl UndoBuffer {
    fn new(initial: Raster) -> Self {
        Self {
            initial,
            events: Vec::new(),
            redo_buffer: Vec::new(),
        }
    }

    fn undo(&mut self) -> Raster {
        let mut raster = self.initial.clone();
        if let Some(event) = self.events.pop() {
            self.redo_buffer.push(event);
            for event in &self.events {
                raster.commit_event(event);
            }
        }
        raster
    }

    fn redo(&mut self) -> Raster {
        let mut raster = self.initial.clone();
        if let Some(event) = self.redo_buffer.pop() {
            self.events.push(event);
        }
        for event in &self.events {
            raster.commit_event(event);
        }
        raster
    }

    fn commit_event(&mut self, event: DrawingEventWithRenderCell) {
        self.events.push(event);
        self.redo_buffer.clear();
    }
}

#[derive(Serialize, Deserialize)]
struct LivePaths {
    palette_path: PathBuf,
    output_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct DrawingState {
    palette_indices: PaletteIndices,
    tools: Vec<Tool>,
    tool_index: usize,
    canvas_state: Raster,
    current_event: Option<DrawingEvent>,
    undo_buffer: UndoBuffer,
    eyedrop_render_cell: Option<RenderCell>,
    fg_opacity: u8,
    bg_opacity: u8,
    palette_hover: PaletteIndices,
    tool_hover: Option<usize>,
    canvas_hover: Option<Coord>,
}

impl DrawingState {
    fn new() -> Self {
        let canvas_state = Raster::new(Size::new(100, 80));
        let undo_buffer = UndoBuffer::new(canvas_state.clone());
        Self {
            palette_indices: Default::default(),
            tools: Tool::all(),
            tool_index: 0,
            canvas_state,
            current_event: None,
            undo_buffer,
            eyedrop_render_cell: None,
            fg_opacity: 255,
            bg_opacity: 255,
            palette_hover: Default::default(),
            tool_hover: None,
            canvas_hover: None,
        }
    }

    fn load<P: AsRef<Path>>(path: P) -> Self {
        use std::io::Read;
        let mut file = File::open(path).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        bincode::deserialize(&data).unwrap()
    }
}

struct AppData {
    live_paths: LivePaths,
    palette: Palette,
    drawing_state: DrawingState,
}

impl AppData {
    fn new_with_live_paths(live_paths: LivePaths, input_path: Option<PathBuf>) -> Self {
        let palette = Palette::load(live_paths.palette_path.as_path()).unwrap();
        let drawing_state = if let Some(input_path) = input_path {
            DrawingState::load(input_path)
        } else {
            DrawingState::new()
        };
        Self {
            live_paths,
            palette,
            drawing_state,
        }
    }

    fn get_ch(&self) -> Option<char> {
        self.drawing_state
            .palette_indices
            .ch?
            .option()
            .map(|i| self.palette.ch[i])
    }

    fn get_fg(&self) -> Option<Rgba32> {
        self.drawing_state
            .palette_indices
            .fg?
            .option()
            .map(|i| self.palette.fg[i].to_rgba32(self.drawing_state.fg_opacity))
    }

    fn get_bg(&self) -> Option<Rgba32> {
        self.drawing_state
            .palette_indices
            .bg?
            .option()
            .map(|i| self.palette.bg[i].to_rgba32(self.drawing_state.bg_opacity))
    }

    fn current_render_cell(&self) -> RenderCell {
        self.drawing_state
            .eyedrop_render_cell
            .unwrap_or_else(|| RenderCell {
                character: self.get_ch(),
                style: Style::default()
                    .with_foreground_option(self.get_fg())
                    .with_background_option(self.get_bg()),
            })
    }

    fn current_tool(&self) -> Tool {
        self.drawing_state.tools[self.drawing_state.tool_index]
    }

    fn commit_current_event(&mut self) {
        if let Some(drawing_event) = self.drawing_state.current_event.take() {
            let event = DrawingEventWithRenderCell {
                drawing_event,
                render_cell: self.current_render_cell(),
            };
            self.drawing_state.canvas_state.commit_event(&event);
            self.drawing_state.undo_buffer.commit_event(event);
        }
    }

    fn undo(&mut self) {
        self.drawing_state.canvas_state = self.drawing_state.undo_buffer.undo();
    }

    fn redo(&mut self) {
        self.drawing_state.canvas_state = self.drawing_state.undo_buffer.redo();
    }

    fn save(&self) {
        // TODO handle errors
        use std::io::Write;
        let mut file = File::create(self.live_paths.output_path.as_path()).unwrap();
        let data = bincode::serialize(&self.drawing_state).unwrap();
        file.write_all(&data).unwrap();
        println!("wrote to {}", self.live_paths.output_path.to_str().unwrap());
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
    type State = AppData;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        {
            let preview_cell = state.current_render_cell();
            let preview_cell = preview_cell
                .with_foreground_option(preview_cell.foreground().map(|c| c.with_a(255)))
                .with_background_option(preview_cell.background().map(|c| c.with_a(255)));
            fb.set_cell_relative_to_ctx(ctx, Coord::new(0, 1), 0, preview_cell);
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
            let style = if state.drawing_state.palette_indices.ch == Some(PaletteIndex::None) {
                select_style
            } else if state.drawing_state.palette_hover.ch == Some(PaletteIndex::None) {
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
            let style = if state.drawing_state.palette_indices.fg == Some(PaletteIndex::None) {
                select_style
            } else if state.drawing_state.palette_hover.fg == Some(PaletteIndex::None) {
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
            let style = if state.drawing_state.palette_indices.bg == Some(PaletteIndex::None) {
                select_style
            } else if state.drawing_state.palette_hover.bg == Some(PaletteIndex::None) {
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
            let style = if Some(PaletteIndex::Index(i)) == state.drawing_state.palette_indices.ch {
                select_style
            } else if Some(PaletteIndex::Index(i)) == state.drawing_state.palette_hover.ch {
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
            let character =
                if Some(PaletteIndex::Index(i)) == state.drawing_state.palette_indices.fg {
                    Some('*')
                } else if Some(PaletteIndex::Index(i)) == state.drawing_state.palette_hover.fg {
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
            let character =
                if Some(PaletteIndex::Index(i)) == state.drawing_state.palette_indices.bg {
                    Some('*')
                } else if Some(PaletteIndex::Index(i)) == state.drawing_state.palette_hover.bg {
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
            let ctx = ctx.add_x(self.preview_offset());
            let ctx = ctx.add_x(self.palette_x_offset());
            let ch_bb = ctx
                .bounding_box
                .set_height(1)
                .set_width(state.palette.ch.len() as u32 + 1);
            let fg_bb = ctx
                .bounding_box
                .add_y(1)
                .set_height(1)
                .set_width(state.palette.fg.len() as u32 + 1);
            let bg_bb = ctx
                .bounding_box
                .add_y(2)
                .set_height(1)
                .set_width(state.palette.bg.len() as u32 + 1);
            fn coord_to_index(c: Coord) -> PaletteIndex {
                if c.x == 0 {
                    PaletteIndex::None
                } else {
                    PaletteIndex::Index(c.x as usize - 1)
                }
            }
            match mouse_input {
                MouseInput::MouseMove { coord, .. } => {
                    state.drawing_state.palette_hover.ch =
                        ch_bb.coord_absolute_to_relative(coord).map(coord_to_index);
                    state.drawing_state.palette_hover.fg =
                        fg_bb.coord_absolute_to_relative(coord).map(coord_to_index);
                    state.drawing_state.palette_hover.bg =
                        bg_bb.coord_absolute_to_relative(coord).map(coord_to_index);
                }
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord,
                } => {
                    if let Some(coord) = ch_bb.coord_absolute_to_relative(coord) {
                        state.drawing_state.palette_indices.ch = Some(coord_to_index(coord));
                    }
                    if let Some(coord) = fg_bb.coord_absolute_to_relative(coord) {
                        state.drawing_state.palette_indices.fg = Some(coord_to_index(coord));
                    }
                    if let Some(coord) = bg_bb.coord_absolute_to_relative(coord) {
                        state.drawing_state.palette_indices.bg = Some(coord_to_index(coord));
                    }
                    state.drawing_state.eyedrop_render_cell = None;
                }
                _ => (),
            }
        }
    }
    fn size(&self, _state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(40, 3)
    }
}

struct OpacityComponent {
    fg_label: text::StyledString,
    bg_label: text::StyledString,
}

impl OpacityComponent {
    fn new() -> Self {
        Self {
            fg_label: text::StyledString::plain_text("fg ".to_string()),
            bg_label: text::StyledString::plain_text("bg ".to_string()),
        }
    }
}

impl Component for OpacityComponent {
    type Output = Option<PopUp>;
    type State = AppData;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        {
            let ctx = ctx.add_y(1);
            self.fg_label.render(&(), ctx, fb);
            let ctx = ctx.add_x(self.fg_label.string.len() as i32);
            text::StyledString::plain_text(format!("{}", state.drawing_state.fg_opacity)).render(
                &(),
                ctx,
                fb,
            );
        }
        {
            let ctx = ctx.add_y(2);
            self.bg_label.render(&(), ctx, fb);
            let ctx = ctx.add_x(self.bg_label.string.len() as i32);
            text::StyledString::plain_text(format!("{}", state.drawing_state.bg_opacity)).render(
                &(),
                ctx,
                fb,
            );
        }
    }
    fn update(&mut self, _state: &mut Self::State, ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            let mouse_input = mouse_input.relative_to_coord(ctx.top_left());
            match mouse_input {
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord: Coord { x: _, y: 1 },
                } => {
                    return Some(PopUp::FgOpacity);
                }
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord: Coord { x: _, y: 2 },
                } => {
                    return Some(PopUp::BgOpacity);
                }
                _ => (),
            }
        }
        None
    }
    fn size(&self, _state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(40, 3)
    }
}

struct ToolsComponent;

impl Component for ToolsComponent {
    type Output = ();
    type State = AppData;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        for (i, tool) in state.drawing_state.tools.iter().enumerate() {
            let ctx = ctx.add_y(i as i32);
            if i == state.drawing_state.tool_index {
                text::StyledString::plain_text(format!("*{}*", tool)).render(&(), ctx, fb);
            } else if Some(i) == state.drawing_state.tool_hover {
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
            match mouse_input {
                MouseInput::MouseMove { coord, .. } => {
                    state.drawing_state.tool_hover = ctx
                        .bounding_box
                        .coord_absolute_to_relative(coord)
                        .map(|c| c.y as usize);
                }
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord,
                } => {
                    if let Some(coord) = ctx.bounding_box.coord_absolute_to_relative(coord) {
                        state.drawing_state.tool_index = coord.y as usize;
                    }
                }
                _ => (),
            }
        }
    }
    fn size(&self, state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(10, state.drawing_state.tools.len() as u32)
    }
}

struct CanvasComponent;

impl Component for CanvasComponent {
    type Output = ();
    type State = AppData;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        for (coord, &cell) in state.drawing_state.canvas_state.grid.enumerate() {
            let mut cell = cell;
            if Some(coord) == state.drawing_state.canvas_hover {
                cell.style.background = if let Some(background) = cell.background() {
                    Some(background.saturating_scalar_mul_div(4, 3))
                } else {
                    Some(Rgba32::new_grey(127))
                };
            }
            fb.set_cell_relative_to_ctx(ctx, coord, 0, cell);
        }
        if let Some(current_event) = state.drawing_state.current_event.as_ref() {
            current_event.preview(
                &state.drawing_state.canvas_state,
                state.current_render_cell(),
                ctx.add_depth(1),
                fb,
            );
        }
    }
    fn update(&mut self, state: &mut Self::State, ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            state.drawing_state.canvas_hover = ctx
                .bounding_box
                .coord_absolute_to_relative(mouse_input.coord());
            if state.current_tool() == Tool::Eyedrop {
                match mouse_input {
                    MouseInput::MousePress {
                        button: MouseButton::Left,
                        coord,
                    }
                    | MouseInput::MouseMove {
                        button: Some(MouseButton::Left),
                        coord,
                    } => {
                        if let Some(coord) = ctx.bounding_box.coord_absolute_to_relative(coord) {
                            if let Some(&render_cell) =
                                state.drawing_state.canvas_state.grid.get(coord)
                            {
                                state.drawing_state.eyedrop_render_cell = Some(render_cell);
                                state.drawing_state.palette_indices.ch = None;
                                state.drawing_state.palette_indices.fg = None;
                                state.drawing_state.palette_indices.bg = None;
                            }
                        }
                    }
                    _ => (),
                }
            } else {
                match mouse_input {
                    MouseInput::MousePress {
                        button: MouseButton::Left,
                        coord,
                    } => {
                        if let Some(coord) = ctx.bounding_box.coord_absolute_to_relative(coord) {
                            state.drawing_state.current_event =
                                state.current_tool().new_event(coord);
                        }
                    }
                    _ => (),
                }
            }
        }
    }
    fn size(&self, state: &Self::State, ctx: Ctx) -> Size {
        state
            .drawing_state
            .canvas_state
            .grid
            .size()
            .pairwise_min(ctx.bounding_box.size())
    }
}

struct GuiComponent {
    palette: Border<PaletteComponent>,
    opacity: Border<OpacityComponent>,
    tools: Border<ToolsComponent>,
    canvas: Border<CanvasComponent>,
}

struct GuiChildCtxs<'a> {
    palette: Ctx<'a>,
    opacity: Ctx<'a>,
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
        let opacity = Self::border(OpacityComponent::new(), "Opacity");
        let tools = Self::border(ToolsComponent, "Tools");
        let canvas = Self::border(CanvasComponent, "Canvas");
        Self {
            palette,
            opacity,
            tools,
            canvas,
        }
    }

    fn child_ctxs<'a>(&self, state: &AppData, ctx: Ctx<'a>) -> GuiChildCtxs<'a> {
        let palette_size = self.palette.size(state, ctx);
        let opacity_size = self.opacity.size(state, ctx);
        let tools_size = self.tools.size(state, ctx);
        let palette =
            ctx.add_y(ctx.bounding_box.size().height() as i32 - palette_size.height() as i32);
        let opacity = palette
            .add_x(palette_size.width() as i32)
            .set_width(opacity_size.width());
        let height_above_palette =
            (ctx.bounding_box.size().height() as i32 - palette_size.height() as i32) as u32;
        let tools = ctx.set_size(tools_size);
        let canvas = ctx
            .set_height(height_above_palette)
            .add_x(tools_size.width() as i32);
        GuiChildCtxs {
            palette,
            opacity,
            tools,
            canvas,
        }
    }
}

impl Component for GuiComponent {
    type Output = Option<PopUp>;
    type State = AppData;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        let ctxs = self.child_ctxs(state, ctx);
        self.palette.render(state, ctxs.palette, fb);
        self.opacity.render(state, ctxs.opacity, fb);
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
            } else {
                state.drawing_state.palette_hover.ch = None;
                state.drawing_state.palette_hover.fg = None;
                state.drawing_state.palette_hover.bg = None;
            }
            if ctxs.tools.bounding_box.contains_coord(mouse_input.coord()) {
                self.tools.update(state, ctxs.tools, event)
            } else {
                state.drawing_state.tool_hover = None;
            }
            if ctxs.canvas.bounding_box.contains_coord(mouse_input.coord()) {
                self.canvas.update(state, ctxs.canvas, event)
            } else {
                state.drawing_state.canvas_hover = None;
            }
            if ctxs
                .opacity
                .bounding_box
                .contains_coord(mouse_input.coord())
            {
                if let Some(popup) = self.opacity.update(state, ctxs.opacity, event) {
                    return Some(popup);
                }
            }
            match mouse_input {
                MouseInput::MouseMove {
                    button: Some(MouseButton::Left),
                    coord,
                } => {
                    if let Some(current_event) = state.drawing_state.current_event.as_mut() {
                        let border_padding_top_left = Coord::new(
                            self.canvas.style.padding.left as i32 + 1,
                            self.canvas.style.padding.top as i32 + 1,
                        );
                        let coord =
                            coord - ctxs.canvas.bounding_box.top_left() - border_padding_top_left;
                        current_event.mouse_move(coord);
                    }
                }
                MouseInput::MouseRelease { .. } => {
                    state.commit_current_event();
                }
                _ => (),
            }
        } else if let Some(keyboard_input) = event.keyboard_input() {
            match keyboard_input {
                KeyboardInput::Char('u') => state.undo(),
                KeyboardInput::Char('r') => state.redo(),
                KeyboardInput::Char('s') => state.save(),
                _ => (),
            }
        }
        None
    }
    fn size(&self, _state: &Self::State, ctx: Ctx) -> Size {
        ctx.bounding_box.size()
    }
}

enum PopUp {
    FgOpacity,
    BgOpacity,
}

enum AppState {
    Ui,
    PopUp(PopUp),
}

fn gui_component() -> CF<Option<PopUp>, AppData> {
    cf(GuiComponent::new())
}

fn opacity_text_field(initial_value: u8) -> CF<Option<OrEscapeOrClickOut<String>>, AppData> {
    cf(TextField::with_initial_string(
        3,
        format!("{}", initial_value),
    ))
    .ignore_state()
    .with_title_horizontal(
        styled_string(
            "Enter foreground opacity (0 - 255):".to_string(),
            Style::plain_text(),
        ),
        1,
    )
    .catch_escape_or_click_out()
}

fn pop_up_style<C: 'static + Component<State = AppData>>(
    component: C,
    title: Option<String>,
) -> CF<C::Output, AppData> {
    use chargrid::border::*;
    cf(component)
        .border(BorderStyle {
            title,
            title_style: Style::plain_text(),
            chars: BorderChars::double_line_light().with_title_separators('╡', '╞'),
            padding: BorderPadding::all(1),
            ..Default::default()
        })
        .fill(Rgba32::new_grey(0))
        .centre()
        .overlay_tint(gui_component(), gridbugs::chargrid::core::TintDim(127), 1)
}

fn opacity_dialog(title: String, initial_value: u8) -> CF<Option<Option<u8>>, AppData> {
    pop_up_style(opacity_text_field(initial_value), Some(title)).map(|result| {
        if let Ok(string) = result {
            if let Ok(opacity) = string.parse::<u8>() {
                return Some(opacity);
            } else {
                println!(
                    "Failed to parse \"{}\" as byte. Enter a number from 0 to 255.",
                    string
                );
            }
        }
        None
    })
}

fn app_loop() -> CF<Option<app::Exit>, AppData> {
    loop_(AppState::Ui, |state| match state {
        AppState::Ui => gui_component().map(AppState::PopUp).continue_(),
        AppState::PopUp(PopUp::FgOpacity) => on_state_then(|state: &mut AppData| {
            opacity_dialog(
                "Foreground Opacity".to_string(),
                state.drawing_state.fg_opacity,
            )
            .map_side_effect(|opacity, data| {
                if let Some(opacity) = opacity {
                    data.drawing_state.fg_opacity = opacity;
                }
            })
            .map_val(|| AppState::Ui)
            .continue_()
        }),
        AppState::PopUp(PopUp::BgOpacity) => on_state_then(|state: &mut AppData| {
            opacity_dialog(
                "Background Opacity".to_string(),
                state.drawing_state.bg_opacity,
            )
            .map_side_effect(|opacity, data| {
                if let Some(opacity) = opacity {
                    data.drawing_state.bg_opacity = opacity;
                }
            })
            .map_val(|| AppState::Ui)
            .continue_()
        }),
    })
}

pub fn app(palette_path: PathBuf, input_path: Option<PathBuf>, output_path: PathBuf) -> App {
    let live_paths = LivePaths {
        palette_path,
        output_path,
    };
    let app_data = AppData::new_with_live_paths(live_paths, input_path);
    app_loop()
        .with_state(app_data)
        .clear_each_frame()
        .exit_on_close()
}
