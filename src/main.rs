use gridbugs::chargrid_wgpu;
use std::path::PathBuf;

mod app;
mod palette;

struct Args {
    palette_path: PathBuf,
    terminal: bool,
    input_path: Option<PathBuf>,
    output_path: PathBuf,
}

impl Args {
    fn parser() -> impl meap::Parser<Item = Self> {
        meap::let_map! {
            let {
                palette_path = opt_req("PATH", "palette").name('p');
                terminal = flag("terminal").name('t').desc("run in a terminal");
                input_path = opt_opt("PATH", "input").name('i');
                output_path = opt_req("PATH", "output").name('o');
            } in {
                Self {
                    palette_path,
                    terminal,
                    input_path,
                    output_path,
                }
            }
        }
    }
}

fn wgpu_context() -> chargrid_wgpu::Context {
    use chargrid_wgpu::*;
    const CELL_SIZE_PX: f64 = 12.;
    Context::new(Config {
        font_bytes: FontBytes {
            normal: include_bytes!("./fonts/PxPlus_IBM_CGAthin.ttf").to_vec(),
            bold: include_bytes!("./fonts/PxPlus_IBM_CGA.ttf").to_vec(),
        },
        title: "Text Paint".to_string(),
        window_dimensions_px: Dimensions {
            width: 1280.,
            height: 840.,
        },
        cell_dimensions_px: Dimensions {
            width: CELL_SIZE_PX,
            height: CELL_SIZE_PX,
        },
        font_scale: Dimensions {
            width: CELL_SIZE_PX,
            height: CELL_SIZE_PX,
        },
        underline_width_cell_ratio: 0.1,
        underline_top_offset_cell_ratio: 0.8,
        resizable: false,
        force_secondary_adapter: false,
    })
}

fn main() {
    use meap::Parser;
    let Args {
        palette_path,
        terminal,
        input_path,
        output_path,
    } = Args::parser().with_help_default().parse_env_or_exit();
    let app = app::app(palette_path, input_path, output_path);
    if terminal {
        use gridbugs::chargrid_ansi_terminal::{Context, XtermTrueColour};
        let context = Context::new().expect("Failed to initialize terminal");
        let colour = XtermTrueColour;
        context.run(app, colour);
    } else {
        let context = wgpu_context();
        context.run(app);
    }
}
