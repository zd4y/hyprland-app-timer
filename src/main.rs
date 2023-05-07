use anyhow::bail;
use app_timer2::Message;
use chrono::Utc;
use focus_monitor::AsyncFocusMonitor;
use ipc_channel::ipc::IpcOneShotServer;
use sqlx::SqlitePool;
use std::env;
use std::time::Instant;
use tokio::sync::mpsc::{self, Receiver, Sender};
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
            if app_timer2::send_save_signal().await.is_ok() {
                bail!("already running")
            }

            let (tx, mut rx) = mpsc::channel(100);

            let tx2 = tx.clone();
            let handle = std::thread::spawn(move || {
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
                        Message::StopSendingSignal => unreachable!(),
                    };
                }
                Ok::<(), anyhow::Error>(())
            });

            run(tx, &mut rx).await?;

            handle.join().expect("failed joining thread handle")?;

            Ok(())
        }
    }
}

async fn run(tx: Sender<Message>, rx: &mut Receiver<Message>) -> anyhow::Result<()> {
    ctrlc::set_handler(move || {
        log::info!("in ctrlc handler, sending signal...");
        tx.blocking_send(Message::StopSendingSignal)
            .expect("Error sending signal to channel")
    })
    .expect("Error setting Ctrl-C handler");

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
    let mut focus_monitor = AsyncFocusMonitor::try_new()?;

    let mut now = Instant::now();
    let interval_period = std::time::Duration::from_secs(60 * 60);
    let mut interval = interval_at(
        tokio::time::Instant::now() + interval_period,
        interval_period,
    );
    let mut last_window: Option<focus_monitor::Window> = None;
    let mut records = vec![];

    loop {
        tokio::select! {
            biased;

            Some(signal) = rx.recv() => match signal {
                Message::StopSendingSignal => {
                    app_timer2::send_stop_signal().await?;
                    // drop(focus_monitor);
                    break;
                },
                Message::Stop => break,
                Message::Save => {
                    save_windows(pool, &mut records).await?;
                }
            },
            _ = interval.tick() => {
                save_windows(pool, &mut records).await?;
            }
            Ok(new_last_window) = focus_monitor.recv() => {
                let duration = now.elapsed();
                now = Instant::now();

                log::info!("received window");
                log::info!("window={:#?}", new_last_window);
                log::info!("{:?} ({:?})", last_window, duration);

                if let Some(window) = last_window.take() {
                    records.push(new_window(window, duration));
                }
                last_window = new_last_window;

                log::info!("records.len()={}", records.len());
                if records.len() >= 100 {
                    save_windows(pool, &mut records).await?;
                }
            }
        }
    }

    // Add the latest window
    if let Some(window) = last_window {
        log::info!("last window: {:?}", window);
        log::info!("records before adding last window {:?}", records);
        let duration = now.elapsed();
        records.push(new_window(window, duration))
    }

    save_windows(pool, &mut records).await?;

    log::info!("exiting program, bye");

    Ok(())
}

async fn save_windows(
    pool: &SqlitePool,
    windows: &mut Vec<app_timer2::Window>,
) -> anyhow::Result<()> {
    app_timer2::save_windows(pool, windows).await?;
    log::info!("windows saved: {:?}", windows);
    windows.clear();
    Ok(())
}

fn new_window(window: focus_monitor::Window, duration: std::time::Duration) -> app_timer2::Window {
    app_timer2::Window {
        datetime: Utc::now() - chrono::Duration::from_std(duration).unwrap(),
        title: window.title,
        class: window.class,
        duration,
    }
}
