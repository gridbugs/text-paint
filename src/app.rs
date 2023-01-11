use crate::config::Config;
use gridbugs::chargrid::control_flow::*;
use std::path::PathBuf;

pub fn app(config_path: PathBuf) -> App {
    let config = Config::load(config_path).unwrap();
    for entry in config.palette() {
        println!("{:#?}", entry);
    }
    unit().ignore_output()
}
