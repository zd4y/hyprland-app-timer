use std::{env, time::Duration};

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Debug)]
pub struct Window {
    pub datetime: DateTime<Utc>,
    pub title: String,
    pub class: (String, String),
    pub duration: Duration,
}

pub async fn get_pool() -> anyhow::Result<SqlitePool> {
    let database_url = env::var("DATABASE_URL")?;
    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;
    log::info!("Connected to sqlite pool {}", database_url);
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn save_windows(pool: &SqlitePool, windows: &[Window]) -> anyhow::Result<()> {
    if windows.is_empty() {
        return Ok(());
    }

    let mut transaction = pool.begin().await?;

    for window in windows {
        let datetime = window.datetime.to_rfc3339();
        let duration = window.duration.as_secs_f64();
        sqlx::query!(
            r#"
                INSERT INTO windows_log (datetime, class_left, class_right, title, duration)
                VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            datetime,
            window.class.0,
            window.class.1,
            window.title,
            duration
        )
        .execute(&mut transaction)
        .await?;
    }

    transaction.commit().await?;

    Ok(())
}

pub async fn get_windows(pool: &SqlitePool) -> Result<Vec<Window>, sqlx::Error> {
    let db_windows = sqlx::query_as!(DbWindow, "SELECT * FROM windows_log")
        .fetch_all(pool)
        .await?;
    Ok(db_windows.into_iter().map(Window::from).collect())
}

pub async fn get_windows_between(
    pool: &SqlitePool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Window>, sqlx::Error> {
    let from = from.to_rfc3339();
    let to = to.to_rfc3339();
    let db_windows = sqlx::query_as!(
        DbWindow,
        "SELECT * FROM windows_log WHERE datetime >= ?1 AND datetime < ?2",
        from,
        to
    )
    .fetch_all(pool)
    .await?;
    Ok(db_windows.into_iter().map(Window::from).collect())
}

struct DbWindow {
    datetime: String,
    class_left: String,
    class_right: String,
    title: String,
    duration: f64,
}

impl From<DbWindow> for Window {
    fn from(db_window: DbWindow) -> Self {
        let datetime = DateTime::parse_from_rfc3339(&db_window.datetime)
            .unwrap()
            .with_timezone(&Utc);
        Self {
            datetime,
            class: (db_window.class_left, db_window.class_right),
            title: db_window.title,
            duration: Duration::from_secs_f64(db_window.duration),
        }
    }
}
