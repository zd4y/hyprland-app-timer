#[cfg(feature = "server")]
pub mod server;

pub mod blocking_client;
mod client;

use chrono::{DateTime, NaiveDate, Utc};
use std::time::Duration;
use xdg::BaseDirectories;

pub use client::Client;

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
