//! 数据库连接（sea-orm，底层 sqlx-sqlite）

use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

use yukina_shared::config::Settings;
use yukina_shared::error::ApiError;

/// 建立连接：SQLite（自动建库）+ 连接池 10；连接后开启 WAL。
pub async fn create_connection(settings: &Settings) -> Result<DatabaseConnection, ApiError> {
    register_vector_extension();
    // mode=rwc：文件不存在时创建
    let url = sqlite_connection_url(&settings.database_path);
    let mut options = ConnectOptions::new(url);
    options.max_connections(10);

    let db = Database::connect(options).await?;

    // WAL 为持久属性
    db.execute_unprepared("PRAGMA journal_mode=WAL").await?;
    Ok(db)
}

// SQLite URLs require forward slashes even when PathBuf returns Windows separators.
fn sqlite_connection_url(path: &str) -> String {
    format!("sqlite://{}?mode=rwc", path.replace('\\', "/"))
}

/// 静态注册，所有 SQLx/SeaORM SQLite 连接都会加载；不开放动态扩展路径。
pub fn register_vector_extension() {
    static REGISTER: std::sync::Once = std::sync::Once::new();
    REGISTER.call_once(|| unsafe {
        // sqlite-vec 官方入口符合 SQLite 的自动扩展 ABI，且与 SQLx 共用 libsqlite3-sys。
        let code = libsqlite3_sys::sqlite3_auto_extension(Some(std::mem::transmute::<
            *const (),
            unsafe extern "C" fn(
                *mut libsqlite3_sys::sqlite3,
                *mut *mut std::os::raw::c_char,
                *const libsqlite3_sys::sqlite3_api_routines,
            ) -> std::os::raw::c_int,
        >(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
        assert_eq!(code, libsqlite3_sys::SQLITE_OK, "注册 sqlite-vec 失败");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_urls_normalize_windows_paths() {
        assert_eq!(
            sqlite_connection_url(r"data\aemeath.db"),
            "sqlite://data/aemeath.db?mode=rwc"
        );
        assert_eq!(
            sqlite_connection_url(r"D:\blog\data\yukina.db"),
            "sqlite://D:/blog/data/yukina.db?mode=rwc"
        );
        assert_eq!(
            sqlite_connection_url("data/yukina.db"),
            "sqlite://data/yukina.db?mode=rwc"
        );
    }

    #[tokio::test]
    async fn native_database_paths_reopen_existing_data() {
        let dir = std::env::temp_dir().join(format!(
            "yukina-db-path-{}",
            yukina_shared::media_names::random_file_name()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("aemeath.db");
        let mut settings = Settings::load();
        settings.database_path = path.to_string_lossy().into_owned();
        let db = create_connection(&settings).await.unwrap();
        db.execute_unprepared("CREATE TABLE retained_data (value INTEGER NOT NULL)")
            .await
            .unwrap();
        db.execute_unprepared("INSERT INTO retained_data VALUES (42)")
            .await
            .unwrap();
        db.close().await.unwrap();
        let reopened = create_connection(&settings).await.unwrap();
        let row = reopened
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                "SELECT value FROM retained_data",
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.try_get::<i64>("", "value").unwrap(), 42);
        drop(row);
        reopened.close().await.unwrap();
        // SQLx's SQLite worker can release its Windows file handle just after close().
        for attempt in 0..40 {
            match std::fs::remove_file(&path) {
                Ok(()) => break,
                Err(error) if attempt < 39 && matches!(error.raw_os_error(), Some(32 | 33)) => {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                Err(error) => panic!("failed to remove test database: {error}"),
            }
        }
        std::fs::remove_dir(dir).unwrap();
    }
}
