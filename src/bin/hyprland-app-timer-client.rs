use hyprland_app_timer::Client;
use std::env;

fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    args.next();

    let client = Client::blocking_new()?;

    match args.next() {
        Some(arg) => match arg.as_str() {
            "stop" => client.blocking_stop(),
            "save" => client.blocking_save(),
            _ => anyhow::bail!("unknown argument received: {arg}"),
        },
        None => anyhow::bail!("required argument missing"),
    }
}
