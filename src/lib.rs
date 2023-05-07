use std::{env, time::Duration};

use chrono::{DateTime, TimeZone, Utc};
pub use sqlx::SqlitePool;
use sqlx::{sqlite::SqliteRow, FromRow, Row};

use ipc_channel::ipc::IpcSender;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Window {
    pub datetime: DateTime<Utc>,
    pub title: String,
    pub class: (String, String),
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

pub async fn get_pool() -> anyhow::Result<SqlitePool> {
    let database_url = env::var("DATABASE_URL")?;
    let pool = SqlitePool::connect(&database_url).await?;
    log::info!("Connected to sqlite pool {}", database_url);
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
        to_save.push((
            datetime,
            &window.class.0,
            &window.class.1,
            &window.title,
            duration,
        ));
    }

    let mut query = String::from(
        "INSERT INTO windows_log (datetime, class_left, class_right, title, duration) VALUES",
    );

    for (index, _) in to_save.iter().enumerate() {
        let x = index * 5;
        query += &format!(
            "\n(?{}, ?{}, ?{}, ?{}, ?{}),",
            x + 1,
            x + 2,
            x + 3,
            x + 4,
            x + 5
        );
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
            .bind(window.4)
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
            SELECT class_right, SUM(duration) as duration
            FROM windows_log WHERE datetime >= ?1 AND datetime < ?2
            GROUP BY class_right
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
            WHERE datetime >= ?1 AND datetime < ?2 AND class_right = ?3
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
            SELECT datetime, title, class_left, class_right, duration
            FROM windows_log WHERE datetime >= ?1 AND datetime < ?2 AND class_right = ?3
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(app)
    .fetch_all(pool)
    .await?;
    Ok(db_apps)
}

impl FromRow<'_, SqliteRow> for Window {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let datetime = row.try_get("datetime")?;
        let datetime = DateTime::parse_from_rfc3339(datetime).unwrap().naive_utc();
        let datetime = Utc.from_utc_datetime(&datetime);
        let class_left = row.try_get("class_left")?;
        let class_right = row.try_get("class_right")?;
        let title = row.try_get("title")?;
        let duration = row.try_get("duration").map(Duration::from_secs_f64)?;
        Ok(Self {
            datetime,
            title,
            class: (class_left, class_right),
            duration,
        })
    }
}

impl FromRow<'_, SqliteRow> for App {
    fn from_row(row: &SqliteRow) -> Result<Self, sqlx::Error> {
        let app = row.try_get("class_right")?;
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

#[derive(Serialize, Deserialize, Debug)]
pub enum Message {
    Save,
    Stop,
    StopSendingSignal,
}

async fn send_signal(msg: Message) -> anyhow::Result<()> {
    let server_name = tokio::fs::read_to_string(env::var("SERVER_URL_FILE")?).await?;
    let tx = IpcSender::connect(server_name)?;
    tx.send(msg)?;
    Ok(())
}

pub async fn send_stop_signal() -> anyhow::Result<()> {
    send_signal(Message::Stop).await
}

pub async fn send_save_signal() -> anyhow::Result<()> {
    send_signal(Message::Save).await
}
