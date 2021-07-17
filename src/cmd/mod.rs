use clap::{Arg, App, SubCommand};
use crate::utils::{VERSION, NAV_VERSION};

mod nav;
pub use nav::run_navigator;

pub enum CmdInput {
    Config(String),
    Navigate,
    None,
}

pub fn scan() -> CmdInput {
    let matches = App::new("Midori")
        .version(VERSION)
        .about("A multi-protocol network relay")
        .author("zephyr <i@zephyr.moe>")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("json config file")
                .help("specify a config file in json format")
                .takes_value(true),
        )
        .subcommand(
            SubCommand::with_name("nav")
                .about("An Interactive config editor")
                .version(NAV_VERSION)
                .author("zephyr <i@zephyr.moe>"),
        )
        .get_matches();
    if let Some(config) = matches.value_of("config") {
        return CmdInput::Config(config.to_string());
    }
    if matches.subcommand_matches("nav").is_some() {
        return CmdInput::Navigate;
    }
    CmdInput::None
}
