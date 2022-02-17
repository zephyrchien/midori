mod io;
mod cmd;
mod dns;
mod relay;
mod error;
mod utils;
mod config;
mod transport;

use std::panic;
use cmd::CmdInput;
use config::GlobalConfig;

fn main() {
    env_logger::init();
    // set global panic hook
    panic::set_hook(Box::new(|panic_info| {
        if let Some(x) = panic_info.payload().downcast_ref::<String>() {
            println!("{}", x);
        } else {
            println!("{:?}", panic_info);
        }
    }));

    match cmd::scan() {
        CmdInput::Config(c) => start_from_config(c),
        CmdInput::Navigate => cmd::run_navigator(),
        CmdInput::None => {}
    }
}

fn start_from_config(c: String) {
    let config = GlobalConfig::from_config_file(&c);
    dns::init_resolver(config.dns_mode.into(), config.dns_servers);
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(relay::run(config.endpoints))
}
