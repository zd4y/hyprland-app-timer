use std::{str::FromStr, time::Duration};

use anyhow::Context;
use chrono::{DateTime, TimeZone, Utc};
pub use sqlx::SqlitePool;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteRow},
    ConnectOptions, FromRow, Row,
};

use ipc_channel::ipc::IpcSender;
use serde::{Deserialize, Serialize};
use xdg::BaseDirectories;

#[derive(Debug)]
pub struct Window {
    pub datetime: DateTime<Utc>,
    pub title: String,
    pub class: String,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct App {
    pub app: String,
    pub duration: Duration,
}

#[derive(Debug)]
pub struct AppDay {
    pub date: String,
    pub duration: Duration,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    Save,
    Stop,
}

pub async fn get_pool() -> anyhow::Result<SqlitePool> {
    let database_url = get_database_url()?;
    let mut options = SqliteConnectOptions::from_str(&database_url)?.create_if_missing(true);
    options.disable_statement_logging();
    let pool = SqlitePool::connect_with(options).await?;
    log::debug!("Connected to sqlite pool {}", database_url);
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn save_windows(pool: &SqlitePool, windows: &[Window]) -> anyhow::Result<()> {
    if windows.is_empty() {
        return Ok(());
    }

    let mut to_save = Vec::new();

    for window in windows {
        let datetime = window.datetime.to_rfc3339();
        let duration = window.duration.as_secs_f64();
        to_save.push((datetime, &window.class, &window.title, duration));
    }

    let mut query =
        String::from("INSERT INTO windows_log (datetime, class, title, duration) VALUES");

    for (index, _) in to_save.iter().enumerate() {
        let i = index * 4;
        query += &format!("\n(?{}, ?{}, ?{}, ?{}),", i + 1, i + 2, i + 3, i + 4,);
    }

    // replace latest `,` with `;`
    let mut query = query.chars();
    query.next_back();
    let query = format!("{};", query.as_str());

    let mut query = sqlx::query(&query);
    for window in to_save {
        query = query
            .bind(window.0)
            .bind(window.1)
            .bind(window.2)
            .bind(window.3)
    }
    query.execute(pool).await?;

    Ok(())
}

pub async fn get_apps_between(
    pool: &SqlitePool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    descending: bool,
) -> Result<Vec<App>, sqlx::Error> {
    let from = from.to_rfc3339();
    let to = to.to_rfc3339();
    dbg!(from.clone(), to.clone());
    let db_apps = sqlx::query_as::<_, App>(&format!(
        r#"
            SELECT class, SUM(duration) as duration
            FROM windows_log WHERE datetime >= ?1 AND datetime < ?2
            GROUP BY class
            ORDER BY duration {}
        "#,
        if descending { "DESC" } else { "ASC" }
    ))
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(db_apps)
}

pub async fn get_daily_app_between(
    pool: &SqlitePool,
    app: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    offset_hours: i32,
    descending: bool,
) -> Result<Vec<AppDay>, sqlx::Error> {
    let from = from.to_rfc3339();
    let to = to.to_rfc3339();
    let db_windows = sqlx::query_as::<_, AppDay>(&format!(
        r#"
            SELECT strftime('%Y-%m-%d', DATETIME(datetime, '{} hours')) as day, SUM(duration) as duration
            FROM windows_log
            WHERE datetime >= ?1 AND datetime < ?2 AND class = ?3
            GROUP BY day
            ORDER BY day {}
        "#,
        offset_hours,
        if descending { "DESC" } else { "ASC" }
    ))
    .bind(from)
    .bind(to)
    .bind(app)
    .fetch_all(pool)
    .await?;
    Ok(db_windows)
}

pub async fn get_app_windows_between(
    pool: &SqlitePool,
    app: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Window>, sqlx::Error> {
    let from = from.to_rfc3339();
    let to = to.to_rfc3339();
    dbg!(from.clone(), to.clone());
    let db_apps = sqlx::query_as::<_, Window>(
        r#"
            SELECT datetime, title, class, duration
            FROM windows_log WHERE datetime >= ?1 AND datetime < ?2 AND class = ?3
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(app)
    .fetch_all(pool)
    .await?;
    Ok(db_apps)
}

pub async fn send_stop_signal() -> anyhow::Result<()> {
    send_signal(Message::Stop).await
}

pub fn send_stop_signal_blocking() -> anyhow::Result<()> {
    send_signal_blocking(Message::Stop)
}

pub async fn send_save_signal() -> anyhow::Result<()> {
    send_signal(Message::Save).await
}

pub fn set_server_name_blocking(name: &str) -> anyhow::Result<()> {
    let server_name_file = get_xdg_dirs()?.place_data_file("server.txt")?;
    std::fs::write(server_name_file, name)?;
    Ok(())
}

impl FromRow<'_, SqliteRow> for Window {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let datetime = row.try_get("datetime")?;
        let datetime = DateTime::parse_from_rfc3339(datetime).unwrap().naive_utc();
        let datetime = Utc.from_utc_datetime(&datetime);
        let class = row.try_get("class")?;
        let title = row.try_get("title")?;
        let duration = row.try_get("duration").map(Duration::from_secs_f64)?;
        Ok(Self {
            datetime,
            title,
            class,
            duration,
        })
    }
}

impl FromRow<'_, SqliteRow> for App {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let app = row.try_get("class")?;
        let duration = row.try_get("duration").map(Duration::from_secs_f64)?;
        Ok(Self { app, duration })
    }
}

impl FromRow<'_, SqliteRow> for AppDay {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let date = row.try_get("day")?;
        let duration = row.try_get("duration").map(Duration::from_secs_f64)?;
        Ok(Self { date, duration })
    }
}

async fn send_signal(msg: Message) -> anyhow::Result<()> {
    let server_name = get_server_name().await?;
    let tx = IpcSender::connect(server_name)?;
    tx.send(msg)?;
    Ok(())
}

fn send_signal_blocking(msg: Message) -> anyhow::Result<()> {
    let server_name = get_server_name_blocking()?;
    let tx = IpcSender::connect(server_name)?;
    tx.send(msg)?;
    Ok(())
}

fn get_database_url() -> anyhow::Result<String> {
    let path = get_xdg_dirs()?.place_data_file("apps.db")?;
    let path_str = path.to_str().context("database path is not unicode")?;
    Ok(format!("sqlite:{path_str}"))
}

async fn get_server_name() -> anyhow::Result<String> {
    let server_name_file = get_xdg_dirs()?.place_data_file("server.txt")?;
    Ok(tokio::fs::read_to_string(server_name_file).await?)
}

fn get_server_name_blocking() -> anyhow::Result<String> {
    let server_name_file = get_xdg_dirs()?.place_data_file("server.txt")?;
    Ok(std::fs::read_to_string(server_name_file)?)
}

fn get_xdg_dirs() -> anyhow::Result<BaseDirectories> {
    Ok(BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"))?)
}
