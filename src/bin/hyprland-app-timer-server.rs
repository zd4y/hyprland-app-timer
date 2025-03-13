use hyprland_app_timer::{Client, Server};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    if Client::new().await?.ping().await.is_ok() {
        anyhow::bail!("already running")
    }

    Server::new().await?.run().await
}
