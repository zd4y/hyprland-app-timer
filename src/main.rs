use chrono::Utc;
use focus_monitor::AsyncFocusMonitor;
use sqlx::SqlitePool;
use std::time::Instant;
use tokio::sync::mpsc::{self, Receiver};

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::channel(100);
    ctrlc::set_handler(move || {
        log::info!("in ctrlc handler, sending signal...");
        tx.blocking_send(())
            .expect("Error sending signal to channel")
    })
    .expect("Error setting Ctrl-C handler");

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("<{d} {l}> - {m}\n")))
        .build("/home/alejandro/app-timer2.log")?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    log4rs::init_config(config)?;

    let pool = app_timer2::get_pool().await?;
    run(&pool, &mut rx).await?;

    Ok(())
}

async fn run(pool: &SqlitePool, rx: &mut Receiver<()>) -> anyhow::Result<()> {
    let mut focus_monitor = AsyncFocusMonitor::try_new()?;

    let mut now = Instant::now();
    let mut last_window: Option<focus_monitor::Window> = None;
    let mut records = vec![];

    loop {
        tokio::select! {
            biased;

            _ = rx.recv() => {
                log::info!("received signal, breaking loop...");
                break
            },
            Ok(new_last_window) = focus_monitor.recv() => {
                let duration = now.elapsed();
                log::info!("received window");
                log::info!("window={:#?}", new_last_window);
                log::info!("{:?} ({:?})", last_window, duration);
                if let Some(window) = last_window.take() {
                    records.push(new_window(window, duration));
                }
                now = Instant::now();
                last_window = new_last_window;

                if records.len() >= 1000 {
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
