#[cfg(feature = "db")]
pub mod blocking_db;

#[cfg(feature = "db")]
mod db;

#[cfg(feature = "client")]
mod client;

#[cfg(feature = "server")]
mod server;

#[cfg(any(feature = "server", feature = "client"))]
use std::path::PathBuf;

use xdg::BaseDirectories;

#[cfg(feature = "client")]
pub use client::Client;

#[cfg(feature = "db")]
pub use db::{AppUsage, AppUsageDay, SqliteDB, Window};

#[cfg(feature = "server")]
pub use server::Server;

fn get_xdg_dirs() -> anyhow::Result<BaseDirectories> {
    Ok(BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"))?)
}

#[repr(u8)]
#[derive(Debug)]
enum Message {
    Ping,
    Save,
    Stop,
}

impl TryFrom<u8> for Message {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Message::Ping as u8 => Ok(Message::Ping),
            x if x == Message::Save as u8 => Ok(Message::Save),
            x if x == Message::Stop as u8 => Ok(Message::Stop),
            _ => Err(()),
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
enum ResponseMessage {
    Success,
    UnknownMessage,
    Failed,
}

impl TryFrom<u8> for ResponseMessage {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == ResponseMessage::Success as u8 => Ok(ResponseMessage::Success),
            x if x == ResponseMessage::UnknownMessage as u8 => Ok(ResponseMessage::UnknownMessage),
            x if x == ResponseMessage::Failed as u8 => Ok(ResponseMessage::Failed),
            _ => Err(()),
        }
    }
}

#[cfg(all(feature = "tokio", any(feature = "server", feature = "client")))]
async fn get_socket_path() -> anyhow::Result<PathBuf> {
    tokio::task::spawn_blocking(blocking_get_socket_path).await?
}

#[cfg(any(feature = "server", feature = "client"))]
fn blocking_get_socket_path() -> anyhow::Result<PathBuf> {
    Ok(crate::get_xdg_dirs()?.place_runtime_file("sock")?)
}
