use gridbugs::chargrid_wgpu;
use std::path::PathBuf;

mod app;
mod config;

struct Args {
    config_path: PathBuf,
    terminal: bool,
}

impl Args {
    fn parser() -> impl meap::Parser<Item = Self> {
        meap::let_map! {
            let {
                config_path = opt_req("PATH", "config");
                terminal = flag("terminal").desc("run in a terminal");
            } in {
                Self {
                    config_path,
                    terminal,
                }
            }
        }
    }
}

fn wgpu_context() -> chargrid_wgpu::Context {
    use chargrid_wgpu::*;
    const CELL_SIZE_PX: f64 = 16.;
    Context::new(Config {
        font_bytes: FontBytes {
            normal: include_bytes!("./fonts/PxPlus_IBM_CGAthin.ttf").to_vec(),
            bold: include_bytes!("./fonts/PxPlus_IBM_CGA.ttf").to_vec(),
        },
        title: "Gridbugs Roguelike Tutorial".to_string(),
        window_dimensions_px: Dimensions {
            width: 960.,
            height: 720.,
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
        config_path,
        terminal,
    } = Args::parser().with_help_default().parse_env_or_exit();
    let app = app::app(config_path);
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
