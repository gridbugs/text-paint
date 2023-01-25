use crate::config::Config;
use gridbugs::chargrid::control_flow::*;
use std::path::PathBuf;

pub fn app(config_path: PathBuf) -> App {
    let config = Config::load(config_path).unwrap();
    println!("{:#?}", config);
    unit()
        .ignore_output()
        .catch_escape() // Catch the escape event so we can exit on escape.
        .map(|res| match res {
            Err(Escape) => app::Exit, // Exit the program when escape is pressed.
            Ok(output) => output,     // Other outputs are simply returned.
        })
        .clear_each_frame()
}
