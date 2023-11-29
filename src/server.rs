use std::{thread, time::Instant};

use chrono::Utc;
use hyprland::event_listener::{EventListener, WindowEventData};
use ipc_channel::ipc::{IpcOneShotServer, IpcSender};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::{sync::mpsc, time::interval_at};

pub struct Server {
    pool: SqlitePool,
    rx: mpsc::Receiver<Message>,
}

impl Server {
    pub async fn new() -> anyhow::Result<Server> {
        let (tx, rx) = mpsc::channel(1);
        let tx2 = tx.clone();
        thread::spawn(move || loop {
            let (ipc_server, ipc_server_name) =
                IpcOneShotServer::new().expect("failed to create ipc server");
            Server::save_ipc_server_name(&ipc_server_name).expect("failed to save ipc server name");
            let (_, message) = ipc_server
                .accept()
                .expect("failed to accept message from ipc server");
            match message {
                Message::Ping => {}
                Message::Save => {
                    tx2.blocking_send(Message::Save)
                        .expect("failed to send message");
                }
                Message::Stop => {
                    tx2.blocking_send(Message::Stop)
                        .expect("failed to send message");
                    break;
                }
            };
        });
        let pool = crate::get_pool().await?;
        Ok(Server { pool, rx })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let (windows_sender, mut windows_receiver) = mpsc::channel(100);
        let windows_sender2 = windows_sender.clone();

        let mut event_listener = EventListener::new();
        event_listener.add_active_window_change_handler(move |data| {
            windows_sender
                .blocking_send(data)
                .expect("failed sending window");
        });
        event_listener.add_window_close_handler(move |_| {
            windows_sender2
                .blocking_send(None)
                .expect("failed sending window");
        });
        thread::spawn(move || {
            event_listener
                .start_listener()
                .expect("error starting listener");
        });

        let mut now = Instant::now();
        let interval_period = std::time::Duration::from_secs(60 * 60);
        let mut interval = interval_at(
            tokio::time::Instant::now() + interval_period,
            interval_period,
        );
        let mut last_window: Option<WindowEventData> = None;
        let mut records = vec![];

        loop {
            tokio::select! {
                biased;

                Some(signal) = self.rx.recv() => match signal {
                    Message::Ping => {}
                    Message::Save => {
                        save_windows(&self.pool, &mut records).await?;
                    }
                    Message::Stop => break,
                },
                _ = interval.tick() => {
                    save_windows(&self.pool, &mut records).await?;
                }
                Some(new_last_window) = windows_receiver.recv() => {
                    let duration = now.elapsed();
                    now = Instant::now();

                    log::debug!("received window: {:?} ||| last_window: {:?} ({:?})", new_last_window, last_window, duration);

                    if let Some(window) = last_window.take() {
                        records.push(new_window(window, duration));
                    }
                    last_window = new_last_window;

                    if records.len() >= 100 {
                        save_windows(&self.pool, &mut records).await?;
                    }
                }
            }
        }

        // Add the last window
        if let Some(window) = last_window {
            log::debug!("last window: {:?}", window);
            log::debug!("records before adding last window {:?}", records);
            let duration = now.elapsed();
            records.push(new_window(window, duration))
        }

        save_windows(&self.pool, &mut records).await?;

        log::debug!("exiting program, bye");

        Ok(())
    }

    pub async fn ping() -> anyhow::Result<()> {
        Server::send_signal(Message::Ping).await
    }

    pub async fn stop() -> anyhow::Result<()> {
        Server::send_signal(Message::Stop).await
    }

    pub fn stop_blocking() -> anyhow::Result<()> {
        Server::send_signal_blocking(Message::Stop)
    }

    pub async fn save() -> anyhow::Result<()> {
        Server::send_signal(Message::Save).await
    }

    async fn send_signal(msg: Message) -> anyhow::Result<()> {
        let server_name = Server::ipc_server_name().await?;
        let tx = IpcSender::connect(server_name)?;
        tx.send(msg)?;
        Ok(())
    }

    fn send_signal_blocking(msg: Message) -> anyhow::Result<()> {
        let server_name = Server::ipc_server_name_blocking()?;
        let tx = IpcSender::connect(server_name)?;
        tx.send(msg)?;
        Ok(())
    }

    async fn ipc_server_name() -> anyhow::Result<String> {
        let server_name_file = crate::get_xdg_dirs()?.place_data_file("server.txt")?;
        Ok(tokio::fs::read_to_string(server_name_file).await?)
    }

    fn ipc_server_name_blocking() -> anyhow::Result<String> {
        let server_name_file = crate::get_xdg_dirs()?.place_data_file("server.txt")?;
        Ok(std::fs::read_to_string(server_name_file)?)
    }

    fn save_ipc_server_name(name: &str) -> anyhow::Result<()> {
        let server_name_file = crate::get_xdg_dirs()?.place_data_file("server.txt")?;
        std::fs::write(server_name_file, name)?;
        Ok(())
    }
}

async fn save_windows(pool: &SqlitePool, windows: &mut Vec<crate::Window>) -> anyhow::Result<()> {
    crate::save_windows(pool, windows).await?;
    log::debug!("windows saved: {:?}", windows);
    windows.clear();
    Ok(())
}

fn new_window(window_event_data: WindowEventData, duration: std::time::Duration) -> crate::Window {
    crate::Window {
        datetime: Utc::now() - chrono::Duration::from_std(duration).unwrap(),
        title: window_event_data.window_title,
        class: window_event_data.window_class,
        duration,
    }
}

#[derive(Serialize, Deserialize, Debug)]
enum Message {
    Ping,
    Save,
    Stop,
}
