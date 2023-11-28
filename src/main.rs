use anyhow::bail;
use hyprland_app_timer::server::Server;
use std::{
    env,
    sync::atomic::{AtomicBool, Ordering},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    args.next();

    match args.next() {
        Some(arg) => match arg.as_str() {
            "stop" => Server::stop().await,
            "save" => Server::save().await,
            _ => {
                bail!("unknown argument received: {arg}")
            }
        },
        None => {
            if Server::ping().await.is_ok() {
                bail!("already running")
            }

            run().await?;

            Ok(())
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let ctrlc_handled = AtomicBool::new(false);
    ctrlc::set_handler(move || {
        if !ctrlc_handled.swap(true, Ordering::SeqCst) {
            log::debug!("in ctrlc handler, sending signal...");
            Server::stop_blocking().expect("Error sending stop signal")
        }
    })?;

    env_logger::init();

    let mut server = Server::new().await?;
    server.run().await?;

    Ok(())
}
