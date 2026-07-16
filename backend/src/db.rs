// ── PostgreSQL connection (via PgDog — no client‑side pool needed) ──

// PgDog handles connection pooling on the server side.
// The app just opens connections on demand and closes them when done.

use diesel_async::{AsyncConnection, AsyncPgConnection, SimpleAsyncConnection};

const DEFAULT_DATABASE_URL: &str = "postgres://homepage:homepage@pgdog:6432/homepage";

fn url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

/// Open a new connection to PgDog/Postgres.
/// Close by dropping the returned connection.
pub async fn connect() -> Result<AsyncPgConnection, diesel::ConnectionError> {
    let mut conn = AsyncPgConnection::establish(&url()).await?;
    // Diesel table types use unqualified names; search_path resolves
    // `keys` → `auth.keys`, etc.
    let _ = conn.batch_execute("SET search_path TO public, auth").await;
    Ok(conn)
}

pub async fn ping() -> bool {
    use diesel::sql_types::Integer;
    use diesel_async::RunQueryDsl;

    match connect().await {
        Ok(mut conn) => {
            diesel::select(diesel::dsl::sql::<Integer>("1"))
                .get_result::<i32>(&mut conn)
                .await
                .is_ok()
        },
        Err(e) => {
            crate::log_warn(crate::LogMsg::DbPing {
                ok: false,
                detail: format!("{e}"),
            });
            false
        },
    }
}

/// Verify DB connectivity at startup.
pub async fn init() {
    let ok = ping().await;
    crate::log_info(crate::LogMsg::DbPing {
        ok,
        detail: if ok {
            "PgDog → Postgres connected".into()
        } else {
            "FAILED — check PgDog and Postgres logs".into()
        },
    });
}
