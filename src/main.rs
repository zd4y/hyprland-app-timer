use anyhow::bail;
use hyprland_app_timer::server::Server;
use std::env;
use tokio::signal::{self, unix, unix::SignalKind};

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

            run().await
        }
    }
}

async fn run() -> anyhow::Result<()> {
    tokio::spawn(async {
        let mut hangup =
            unix::signal(SignalKind::hangup()).expect("failed to listen to hangup signal");
        let mut terminate =
            unix::signal(SignalKind::terminate()).expect("failed to listen to terminate signal");
        tokio::select! {
            _ = signal::ctrl_c() => {},
            _ = hangup.recv() => {},
            _ = terminate.recv() => {}
        };
        Server::stop().await.expect("failed to send stop signal")
    });

    env_logger::init();

    Server::new().await?.run().await
}
