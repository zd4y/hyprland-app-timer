use std::{thread, time::Instant};

use anyhow::Context;
use chrono::Utc;
use hyprland::event_listener::{EventListener, WindowEventData};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    signal::{
        self,
        unix::{self, SignalKind},
    },
    sync::mpsc,
    time::interval_at,
};

use crate::{db::Window, Message, ResponseMessage, SqliteDB};

#[derive(Debug)]
pub struct Server {
    db: SqliteDB,
    records: Vec<Window>,
    shutdown_send: mpsc::UnboundedSender<()>,
    shutdown_recv: mpsc::UnboundedReceiver<()>,
}

impl Server {
    pub async fn new() -> anyhow::Result<Server> {
        let db = SqliteDB::new().await?;
        let records = Vec::new();
        let (shutdown_send, shutdown_recv) = mpsc::unbounded_channel();
        Ok(Server {
            db,
            records,
            shutdown_send,
            shutdown_recv,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let (windows_sender, mut windows_receiver) = mpsc::unbounded_channel();
        spawn_event_listener(windows_sender);

        let socket_path = crate::get_socket_path()
            .await
            .context("failed to get socket path")?;
        if let Ok(true) = fs::try_exists(&socket_path).await {
            fs::remove_file(&socket_path)
                .await
                .context("failed to remove socket")?;
        }
        let listener = UnixListener::bind(socket_path).context("failed to bind socket")?;

        let mut hangup =
            unix::signal(SignalKind::hangup()).context("failed to listen to hangup signal")?;
        let mut terminate = unix::signal(SignalKind::terminate())
            .context("failed to listen to terminate signal")?;

        let mut now = Instant::now();
        let interval_period = std::time::Duration::from_secs(60 * 60);
        let mut interval = interval_at(
            tokio::time::Instant::now() + interval_period,
            interval_period,
        );
        let mut last_window: Option<WindowEventData> = None;

        loop {
            tokio::select! {
                biased;

                _ = self.shutdown_recv.recv() => break,
                _ = signal::ctrl_c() => break,
                _ = hangup.recv() => break,
                _ = terminate.recv() => break,
                conn = listener.accept() => match conn {
                    Ok((stream, _)) => {
                        if let Err(err) = self.handle_socket_connection(stream).await {
                            log::error!("an error occurred handling socket connection: {err}");
                        }
                    },
                    Err(err) => {
                        log::error!("failed to accept incoming stream: {err}");
                    }
                },
                _ = interval.tick() => {
                    self.save_windows().await?;
                }
                Some(new_last_window) = windows_receiver.recv() => {
                    let duration = now.elapsed();
                    now = Instant::now();

                    log::debug!("received window: {:?} ||| last_window: {:?} ({:?})", new_last_window, last_window, duration);

                    if let Some(window) = last_window.take() {
                        self.records.push(new_window(window, duration));
                    }
                    last_window = new_last_window;

                    if self.records.len() >= 100 {
                        self.save_windows().await?;
                    }
                }
            }
        }

        // Add the last window
        if let Some(window) = last_window {
            log::debug!("last window: {:?}", window);
            log::debug!("records before adding last window {:?}", self.records);
            let duration = now.elapsed();
            self.records.push(new_window(window, duration))
        }

        self.save_windows().await?;

        log::debug!("exiting program, bye");

        Ok(())
    }

    async fn save_windows(&mut self) -> anyhow::Result<()> {
        self.db.save_windows(&self.records).await?;
        log::debug!("windows saved: {:?}", self.records);
        self.records.clear();
        Ok(())
    }

    async fn handle_socket_connection(&mut self, mut stream: UnixStream) -> anyhow::Result<()> {
        let mut buf: [u8; 1] = [0];
        if let Err(err) = stream.read_exact(&mut buf).await {
            log::error!("stream read failed: {err}");
            return Ok(());
        }
        match Message::try_from(buf[0]) {
            Ok(msg) => {
                log::info!("handling message from socket: {msg:?}");
                if let Err(err) = self.handle_message(msg, &mut stream).await {
                    log::error!("failed handling message: {err}");
                    stream.write_all(&[ResponseMessage::Failed as u8]).await?;
                }
            }
            Err(()) => {
                log::warn!("received unknown message from socket");
                stream
                    .write_all(&[ResponseMessage::UnknownMessage as u8])
                    .await?;
            }
        }
        Ok(())
    }

    async fn handle_message(
        &mut self,
        msg: Message,
        stream: &mut UnixStream,
    ) -> anyhow::Result<()> {
        match msg {
            Message::Ping => {}
            Message::Save => {
                self.save_windows().await?;
            }
            Message::Stop => {
                self.shutdown_send.send(())?;
            }
        }
        stream.write_all(&[ResponseMessage::Success as u8]).await?;
        Ok(())
    }
}

fn new_window(window_event_data: WindowEventData, duration: std::time::Duration) -> Window {
    Window {
        datetime: Utc::now() - chrono::Duration::from_std(duration).unwrap(),
        title: window_event_data.title,
        class: window_event_data.class,
        duration,
    }
}

fn spawn_event_listener(windows_sender: mpsc::UnboundedSender<Option<WindowEventData>>) {
    let windows_sender2 = windows_sender.clone();
    let windows_sender3 = windows_sender.clone();

    thread::spawn(move || {
        let mut event_listener = EventListener::new();
        event_listener.add_active_window_changed_handler(move |data| {
            windows_sender.send(data).expect("failed sending window");
        });
        event_listener.add_window_closed_handler(move |_| {
            windows_sender2.send(None).expect("failed sending window");
        });
        event_listener.add_workspace_added_handler(move |_| {
            windows_sender3.send(None).expect("failed sending window");
        });
        // FIXME: stop the program if this panics
        event_listener
            .start_listener()
            .expect("failed to start event listener")
    });
}
