use std::path::PathBuf;

#[cfg(feature = "tokio")]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

use crate::{Message, ResponseMessage};

#[derive(Debug)]
pub struct Client {
    socket_path: PathBuf,
}

impl Client {
    #[cfg(feature = "tokio")]
    pub async fn new() -> anyhow::Result<Client> {
        let socket_path = crate::get_socket_path().await?;
        Ok(Self { socket_path })
    }

    pub fn blocking_new() -> anyhow::Result<Client> {
        let socket_path = crate::blocking_get_socket_path()?;
        Ok(Self { socket_path })
    }

    #[cfg(feature = "tokio")]
    pub async fn ping(&self) -> anyhow::Result<()> {
        self.send_message(Message::Ping).await
    }

    #[cfg(feature = "tokio")]
    pub async fn stop(&self) -> anyhow::Result<()> {
        self.send_message(Message::Stop).await
    }

    pub fn blocking_stop(&self) -> anyhow::Result<()> {
        self.blocking_send_message(Message::Stop)
    }

    #[cfg(feature = "tokio")]
    pub async fn save(&self) -> anyhow::Result<()> {
        self.send_message(Message::Save).await
    }

    pub fn blocking_save(&self) -> anyhow::Result<()> {
        self.blocking_send_message(Message::Save)
    }

    #[cfg(feature = "tokio")]
    async fn send_message(&self, msg: Message) -> anyhow::Result<()> {
        let mut socket = UnixStream::connect(&self.socket_path).await?;
        socket.write_all(&[msg as u8]).await?;

        let mut buf: [u8; 1] = [0];
        socket.read_exact(&mut buf).await?;

        self.handle_response(&buf)
    }

    fn blocking_send_message(&self, msg: Message) -> anyhow::Result<()> {
        let mut socket = std::os::unix::net::UnixStream::connect(&self.socket_path)?;
        std::io::Write::write_all(&mut socket, &[msg as u8])?;

        let mut buf: [u8; 1] = [0];
        std::io::Read::read_exact(&mut socket, &mut buf)?;

        self.handle_response(&buf)
    }

    fn handle_response(&self, response_buf: &[u8]) -> anyhow::Result<()> {
        match ResponseMessage::try_from(response_buf[0]) {
            Ok(ResponseMessage::Success) => Ok(()),
            Ok(ResponseMessage::UnknownMessage) => {
                anyhow::bail!("unexpected error, server couldn't understand the message")
            }
            Ok(ResponseMessage::Failed) => {
                anyhow::bail!("server error, please check server logs");
            }
            Err(()) => anyhow::bail!("unexpected error, unknown message received from server"),
        }
    }
}
