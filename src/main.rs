use gridbugs::chargrid_wgpu;
use std::path::PathBuf;

mod app;
mod config;
mod parse_colour;

struct Args {
    config_path: PathBuf,
}

impl Args {
    fn parser() -> impl meap::Parser<Item = Self> {
        meap::let_map! {
            let {
                config_path = opt_req("PATH", "config");
            } in {
                Self {
                    config_path,
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
    let Args { config_path } = Args::parser().with_help_default().parse_env_or_exit();
    let context = wgpu_context();
    context.run(app::app(config_path));
}
