use crate::config::Config;
use gridbugs::chargrid::control_flow::*;
use std::path::PathBuf;

pub fn app(config_path: PathBuf) -> App {
    let config = Config::load(config_path).unwrap();
    println!("{:#?}", config);
    unit().ignore_output()
}
