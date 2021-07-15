mod cmd;
mod dns;
mod relay;
mod utils;
mod config;
mod transport;

use cmd::CmdInput;
use config::GlobalConfig;

fn main() {
    match cmd::scan() {
        CmdInput::Config(c) => start_from_config(c),
        CmdInput::Navigate => cmd::run_navigator(),
        CmdInput::None => {}
    }
}

fn start_from_config(c: String) {
    let config = GlobalConfig::from_config_file(&c);
    dns::init_resolver(config.dns_mode.into_strategy());
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(relay::run(config.endpoints))
}
