use anyhow::bail;
use app_timer2::{send_stop_signal_blocking, Message};
use chrono::Utc;
use hyprland::event_listener::{EventListener, WindowEventData};
use ipc_channel::ipc::IpcOneShotServer;
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::{env, thread};
use tokio::sync::mpsc::{self, Receiver};
use tokio::time::interval_at;

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    args.next();

    match args.next() {
        Some(arg) => match arg.as_str() {
            "stop" => app_timer2::send_stop_signal().await,
            "save" => app_timer2::send_save_signal().await,
            _ => {
                bail!("unknown argument received: {arg}")
            }
        },
        None => {
            // TODO: Consider using a new signal (maybe ping)
            if app_timer2::send_save_signal().await.is_ok() {
                bail!("already running")
            }

            let (tx, mut rx) = mpsc::channel(100);

            let tx2 = tx.clone();
            let handle = thread::spawn(move || {
                loop {
                    let (server, server_name) = IpcOneShotServer::<Message>::new()?;
                    {
                        let server_url_file = env::var("SERVER_URL_FILE")?;
                        std::fs::write(server_url_file, server_name)?;
                    }
                    let message = server.accept()?.1;
                    match message {
                        Message::Save => {
                            tx2.blocking_send(Message::Save)?;
                        }
                        Message::Stop => {
                            tx2.blocking_send(Message::Stop)?;
                            break;
                        }
                    };
                }
                Ok::<(), anyhow::Error>(())
            });

            run(&mut rx).await?;

            handle.join().expect("failed joining thread handle")?;

            Ok(())
        }
    }
}

async fn run(rx: &mut Receiver<Message>) -> anyhow::Result<()> {
    let ctrlc_handled = AtomicBool::new(false);
    ctrlc::set_handler(move || {
        if !ctrlc_handled.swap(true, Ordering::SeqCst) {
            log::debug!("in ctrlc handler, sending signal...");
            send_stop_signal_blocking().expect("Error sending stop signal")
        }
    })?;

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("<{d} {l}> - {m}\n")))
        .build(env::var("LOG_FILE")?)?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    log4rs::init_config(config)?;
    log_panics::init();

    let pool = app_timer2::get_pool().await?;
    run_server(&pool, rx).await?;

    Ok(())
}

async fn run_server(pool: &SqlitePool, rx: &mut Receiver<Message>) -> anyhow::Result<()> {
    let (windows_sender, mut windows_receiver) = mpsc::channel(100);
    let windows_sender = Arc::new(windows_sender);
    let windows_sender2 = Arc::clone(&windows_sender);

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

            Some(signal) = rx.recv() => match signal {
                Message::Stop => break,
                Message::Save => {
                    save_windows(pool, &mut records).await?;
                }
            },
            _ = interval.tick() => {
                save_windows(pool, &mut records).await?;
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
                    save_windows(pool, &mut records).await?;
                }
            }
        }
    }

    // Add the latest window
    if let Some(window) = last_window {
        log::debug!("last window: {:?}", window);
        log::debug!("records before adding last window {:?}", records);
        let duration = now.elapsed();
        records.push(new_window(window, duration))
    }

    save_windows(pool, &mut records).await?;

    log::debug!("exiting program, bye");

    Ok(())
}

async fn save_windows(
    pool: &SqlitePool,
    windows: &mut Vec<app_timer2::Window>,
) -> anyhow::Result<()> {
    app_timer2::save_windows(pool, windows).await?;
    log::debug!("windows saved: {:?}", windows);
    windows.clear();
    Ok(())
}

fn new_window(
    window_event_data: WindowEventData,
    duration: std::time::Duration,
) -> app_timer2::Window {
    app_timer2::Window {
        datetime: Utc::now() - chrono::Duration::from_std(duration).unwrap(),
        title: window_event_data.window_title,
        class: (
            window_event_data.window_class.clone(),
            window_event_data.window_class,
        ),
        duration,
    }
}
