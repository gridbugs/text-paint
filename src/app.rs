use crate::palette::Palette;
use gridbugs::{
    chargrid::{self, border::Border, control_flow::*, prelude::*, text},
    grid_2d::Grid,
    line_2d,
    rgb_int::Rgb24,
};
use std::{
    collections::{HashMap, HashSet},
    fmt, iter,
    path::PathBuf,
};

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

type PaletteIndices = PerPalette<Option<PaletteIndex>>;

#[derive(Clone, Copy, PartialEq, Eq)]
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
            fb.set_cell_relative_to_ctx(ctx, coord, 0, render_cell);
        }
    }
}

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
    fn preview(&self, render_cell: RenderCell, ctx: Ctx, fb: &mut FrameBuffer) {
        for coord in line_2d::coords_between(self.start, self.end) {
            fb.set_cell_relative_to_ctx(ctx, coord, 0, render_cell);
        }
    }
}

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
            Self::Line(line) => line.preview(render_cell, ctx, fb),
            Self::Erase(erase) => erase.preview(ctx, fb),
        }
    }
}

#[derive(Clone)]
struct Raster {
    grid: Grid<RenderCell>,
}

impl Raster {
    fn new(size: Size) -> Self {
        let cell = RenderCell {
            character: None,
            style: Style::default(),
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
        ret.style.foreground = blend(top.style.foreground, bottom.style.foreground);
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
            raster_cell.character = None;
            raster_cell.style.background = None;
            raster_cell.style.foreground = None;
            raster_cell.style.bold = None;
            raster_cell.style.underline = None;
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

struct DrawingEventWithRenderCell {
    drawing_event: DrawingEvent,
    render_cell: RenderCell,
}

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

struct AppState {
    palette: Palette,
    palette_indices: PaletteIndices,
    palette_hover: PaletteIndices,
    tools: Vec<Tool>,
    tool_index: usize,
    tool_hover: Option<usize>,
    canvas_state: Raster,
    canvas_hover: Option<Coord>,
    current_event: Option<DrawingEvent>,
    undo_buffer: UndoBuffer,
    eyedrop_render_cell: Option<RenderCell>,
    fg_transparency: u8,
    bg_transparency: u8,
}

impl AppState {
    fn new_with_palette(palette: Palette) -> Self {
        let canvas_state = Raster::new(Size::new(100, 80));
        let undo_buffer = UndoBuffer::new(canvas_state.clone());
        Self {
            palette,
            palette_indices: Default::default(),
            palette_hover: Default::default(),
            tools: Tool::all(),
            tool_index: 0,
            tool_hover: None,
            canvas_state,
            canvas_hover: None,
            current_event: None,
            undo_buffer,
            eyedrop_render_cell: None,
            fg_transparency: 90,
            bg_transparency: 90,
        }
    }

    fn get_ch(&self) -> Option<char> {
        self.palette_indices
            .ch?
            .option()
            .map(|i| self.palette.ch[i])
    }

    fn get_fg(&self) -> Option<Rgba32> {
        self.palette_indices
            .fg?
            .option()
            .map(|i| self.palette.fg[i].to_rgba32(self.fg_transparency))
    }

    fn get_bg(&self) -> Option<Rgba32> {
        self.palette_indices
            .bg?
            .option()
            .map(|i| self.palette.bg[i].to_rgba32(self.bg_transparency))
    }

    fn current_render_cell(&self) -> RenderCell {
        self.eyedrop_render_cell.unwrap_or_else(|| RenderCell {
            character: self.get_ch(),
            style: Style::default()
                .with_foreground_option(self.get_fg())
                .with_background_option(self.get_bg()),
        })
    }

    fn current_tool(&self) -> Tool {
        self.tools[self.tool_index]
    }

    fn commit_current_event(&mut self) {
        if let Some(drawing_event) = self.current_event.take() {
            let event = DrawingEventWithRenderCell {
                drawing_event,
                render_cell: self.current_render_cell(),
            };
            self.canvas_state.commit_event(&event);
            self.undo_buffer.commit_event(event);
        }
    }

    fn undo(&mut self) {
        self.canvas_state = self.undo_buffer.undo();
    }

    fn redo(&mut self) {
        self.canvas_state = self.undo_buffer.redo();
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
            let style = if state.palette_indices.ch == Some(PaletteIndex::None) {
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
            let style = if state.palette_indices.fg == Some(PaletteIndex::None) {
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
            let style = if state.palette_indices.bg == Some(PaletteIndex::None) {
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
            let style = if Some(PaletteIndex::Index(i)) == state.palette_indices.ch {
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
            let character = if Some(PaletteIndex::Index(i)) == state.palette_indices.fg {
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
            let character = if Some(PaletteIndex::Index(i)) == state.palette_indices.bg {
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
                        state.palette_indices.ch = Some(coord_to_index(coord));
                    }
                    if let Some(coord) = fg_bb.coord_absolute_to_relative(coord) {
                        state.palette_indices.fg = Some(coord_to_index(coord));
                    }
                    if let Some(coord) = bg_bb.coord_absolute_to_relative(coord) {
                        state.palette_indices.bg = Some(coord_to_index(coord));
                    }
                    state.eyedrop_render_cell = None;
                }
                _ => (),
            }
        }
    }
    fn size(&self, _state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(40, 3)
    }
}

struct TransparencyComponent {
    fg_label: text::StyledString,
    bg_label: text::StyledString,
}

impl TransparencyComponent {
    fn new() -> Self {
        Self {
            fg_label: text::StyledString::plain_text("fg ".to_string()),
            bg_label: text::StyledString::plain_text("bg ".to_string()),
        }
    }
}

impl Component for TransparencyComponent {
    type Output = ();
    type State = AppState;
    fn render(&self, state: &Self::State, ctx: Ctx, fb: &mut FrameBuffer) {
        let num_digits = 3;
        let slider_left_padding = self.bg_label.string.len() + num_digits + 1;
        let width = ctx.bounding_box.size().width();
        let slider_bar_width = 4;
        let slider_space_width = width - slider_left_padding as u32;
        {
            let slider_offset =
                (state.fg_transparency as u32 * (slider_space_width - slider_bar_width)) / 255;
            let ctx = ctx.add_y(1);
            self.fg_label.render(&(), ctx, fb);
            let ctx = ctx.add_x(self.fg_label.string.len() as i32);
            text::StyledString::plain_text(format!("{}", state.fg_transparency)).render(
                &(),
                ctx,
                fb,
            );
            let ctx = ctx.add_x(num_digits as i32 + 1);
            for i in 0..slider_space_width {
                fb.set_cell_relative_to_ctx(
                    ctx,
                    Coord::new(i as i32, 0),
                    0,
                    RenderCell {
                        character: Some('-'),
                        style: Style::plain_text(),
                    },
                );
            }
            for i in 0..slider_bar_width {
                fb.set_cell_relative_to_ctx(
                    ctx,
                    Coord::new(slider_offset as i32 + i as i32, 0),
                    0,
                    RenderCell {
                        character: Some('█'),
                        style: Style::plain_text(),
                    },
                );
            }
        }
        {
            let slider_offset =
                (state.bg_transparency as u32 * (slider_space_width - slider_bar_width)) / 255;
            let ctx = ctx.add_y(2);
            self.bg_label.render(&(), ctx, fb);
            let ctx = ctx.add_x(self.bg_label.string.len() as i32);
            text::StyledString::plain_text(format!("{}", state.bg_transparency)).render(
                &(),
                ctx,
                fb,
            );
            let ctx = ctx.add_x(num_digits as i32 + 1);
            for i in 0..slider_space_width {
                fb.set_cell_relative_to_ctx(
                    ctx,
                    Coord::new(i as i32, 0),
                    0,
                    RenderCell {
                        character: Some('-'),
                        style: Style::plain_text(),
                    },
                );
            }
            for i in 0..slider_bar_width {
                fb.set_cell_relative_to_ctx(
                    ctx,
                    Coord::new(slider_offset as i32 + i as i32, 0),
                    0,
                    RenderCell {
                        character: Some('█'),
                        style: Style::plain_text(),
                    },
                );
            }
        }
    }
    fn update(&mut self, _state: &mut Self::State, _ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            match mouse_input {
                MouseInput::MousePress {
                    button: MouseButton::Left,
                    coord,
                } => {
                    let _ = coord;
                }
                _ => (),
            }
        }
    }
    fn size(&self, _state: &Self::State, _ctx: Ctx) -> Size {
        Size::new(40, 3)
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
        for (coord, &cell) in state.canvas_state.grid.enumerate() {
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
        if let Some(current_event) = state.current_event.as_ref() {
            current_event.preview(
                &state.canvas_state,
                state.current_render_cell(),
                ctx.add_depth(1),
                fb,
            );
        }
    }
    fn update(&mut self, state: &mut Self::State, ctx: Ctx, event: Event) -> Self::Output {
        if let Some(mouse_input) = event.mouse_input() {
            state.canvas_hover = ctx
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
                            if let Some(&render_cell) = state.canvas_state.grid.get(coord) {
                                state.eyedrop_render_cell = Some(render_cell);
                                state.palette_indices.ch = None;
                                state.palette_indices.fg = None;
                                state.palette_indices.bg = None;
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
                            state.current_event = state.current_tool().new_event(coord);
                        }
                    }
                    _ => (),
                }
            }
        }
    }
    fn size(&self, state: &Self::State, ctx: Ctx) -> Size {
        state
            .canvas_state
            .grid
            .size()
            .pairwise_min(ctx.bounding_box.size())
    }
}

struct GuiComponent {
    palette: Border<PaletteComponent>,
    transparency: Border<TransparencyComponent>,
    tools: Border<ToolsComponent>,
    canvas: Border<CanvasComponent>,
}

struct GuiChildCtxs<'a> {
    palette: Ctx<'a>,
    transparency: Ctx<'a>,
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
        let transparency = Self::border(TransparencyComponent::new(), "Transparency");
        let tools = Self::border(ToolsComponent, "Tools");
        let canvas = Self::border(CanvasComponent, "Canvas");
        Self {
            palette,
            transparency,
            tools,
            canvas,
        }
    }

    fn child_ctxs<'a>(&self, state: &AppState, ctx: Ctx<'a>) -> GuiChildCtxs<'a> {
        let palette_size = self.palette.size(state, ctx);
        let transparency_size = self.transparency.size(state, ctx);
        let tools_size = self.tools.size(state, ctx);
        let palette =
            ctx.add_y(ctx.bounding_box.size().height() as i32 - palette_size.height() as i32);
        let transparency = palette
            .add_x(palette_size.width() as i32)
            .set_width(transparency_size.width());
        let height_above_palette =
            (ctx.bounding_box.size().height() as i32 - palette_size.height() as i32) as u32;
        let tools = ctx.set_size(tools_size);
        let canvas = ctx
            .set_height(height_above_palette)
            .add_x(tools_size.width() as i32);
        GuiChildCtxs {
            palette,
            transparency,
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
        self.transparency.render(state, ctxs.transparency, fb);
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
                state.palette_hover.ch = None;
                state.palette_hover.fg = None;
                state.palette_hover.bg = None;
            }
            if ctxs.tools.bounding_box.contains_coord(mouse_input.coord()) {
                self.tools.update(state, ctxs.tools, event)
            } else {
                state.tool_hover = None;
            }
            if ctxs.canvas.bounding_box.contains_coord(mouse_input.coord()) {
                self.canvas.update(state, ctxs.canvas, event)
            } else {
                state.canvas_hover = None;
            }
            match mouse_input {
                MouseInput::MouseMove {
                    button: Some(MouseButton::Left),
                    coord,
                } => {
                    if let Some(current_event) = state.current_event.as_mut() {
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
                _ => (),
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
