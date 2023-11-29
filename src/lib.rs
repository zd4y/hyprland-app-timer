#[cfg(feature = "server")]
pub mod server;

mod client;

pub use client::Client;

use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
pub use sqlx::SqlitePool;

use xdg::BaseDirectories;

#[derive(Debug)]
pub struct Window {
    pub datetime: DateTime<Utc>,
    pub title: String,
    pub class: String,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct AppUsage {
    pub app: String,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct AppUsageDay {
    pub date_utc: NaiveDate,
    pub duration: Duration,
}

fn get_xdg_dirs() -> anyhow::Result<BaseDirectories> {
    Ok(BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"))?)
}
