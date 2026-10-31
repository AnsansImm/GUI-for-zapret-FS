//! Read browser history through a consistent in-memory SQLite backup.
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    path::Path,
    ptr,
    time::{Duration, Instant},
};

#[link(name = "winsqlite3")]
extern "C" {
    fn sqlite3_open_v2(
        path: *const c_char,
        db: *mut *mut c_void,
        flags: c_int,
        vfs: *const c_char,
    ) -> c_int;
    fn sqlite3_close(db: *mut c_void) -> c_int;
    fn sqlite3_errmsg(db: *mut c_void) -> *const c_char;
    fn sqlite3_busy_timeout(db: *mut c_void, ms: c_int) -> c_int;
    fn sqlite3_backup_init(
        dst: *mut c_void,
        dst_name: *const c_char,
        src: *mut c_void,
        src_name: *const c_char,
    ) -> *mut c_void;
    fn sqlite3_backup_step(backup: *mut c_void, pages: c_int) -> c_int;
    fn sqlite3_backup_finish(backup: *mut c_void) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut c_void,
        sql: *const c_char,
        bytes: c_int,
        stmt: *mut *mut c_void,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut c_void) -> c_int;
    fn sqlite3_finalize(stmt: *mut c_void) -> c_int;
    fn sqlite3_column_count(stmt: *mut c_void) -> c_int;
    fn sqlite3_column_text(stmt: *mut c_void, column: c_int) -> *const u8;
}
struct Db(*mut c_void);
impl Drop for Db {
    fn drop(&mut self) {
        unsafe {
            sqlite3_close(self.0);
        }
    }
}
struct Statement(*mut c_void);
impl Drop for Statement {
    fn drop(&mut self) {
        unsafe {
            sqlite3_finalize(self.0);
        }
    }
}
impl Db {
    fn open(path: &str, flags: i32) -> Result<Self, String> {
        let path = CString::new(path).map_err(|e| e.to_string())?;
        let mut db = ptr::null_mut();
        let rc = unsafe { sqlite3_open_v2(path.as_ptr(), &mut db, flags, ptr::null()) };
        if db.is_null() {
            return Err("Не удалось открыть историю браузера".into());
        }
        let db = Self(db);
        if rc != 0 {
            return Err(db.error());
        }
        unsafe {
            sqlite3_busy_timeout(db.0, 200);
        }
        Ok(db)
    }
    fn error(&self) -> String {
        unsafe {
            CStr::from_ptr(sqlite3_errmsg(self.0))
                .to_string_lossy()
                .into_owned()
        }
    }
    fn query(&self, sql: &str) -> Result<Vec<Vec<String>>, String> {
        let sql = CString::new(sql).map_err(|e| e.to_string())?;
        let mut stmt = ptr::null_mut();
        let rc =
            unsafe { sqlite3_prepare_v2(self.0, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        let stmt = Statement(stmt);
        if rc != 0 {
            return Err(self.error());
        }
        let mut rows = Vec::new();
        loop {
            match unsafe { sqlite3_step(stmt.0) } {
                101 => return Ok(rows),
                100 => {
                    let row = (0..unsafe { sqlite3_column_count(stmt.0) })
                        .map(|column| {
                            let text = unsafe { sqlite3_column_text(stmt.0, column) };
                            if text.is_null() {
                                String::new()
                            } else {
                                unsafe {
                                    CStr::from_ptr(text.cast()).to_string_lossy().into_owned()
                                }
                            }
                        })
                        .collect();
                    rows.push(row);
                }
                _ => return Err(self.error()),
            }
        }
    }
}
pub fn read(path: &Path, firefox: bool) -> Result<Vec<Vec<String>>, String> {
    let source = Db::open(&path.to_string_lossy(), 1)?; // SQLITE_OPEN_READONLY
    let snapshot = Db::open(":memory:", 6)?;
    let name = c"main";
    let backup = unsafe { sqlite3_backup_init(snapshot.0, name.as_ptr(), source.0, name.as_ptr()) };
    if backup.is_null() {
        return Err(snapshot.error());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = loop {
        let rc = unsafe { sqlite3_backup_step(backup, 256) };
        if rc == 101 {
            break Ok(());
        }
        if ![0, 5, 6].contains(&rc) {
            break Err(snapshot.error());
        }
        if Instant::now() >= deadline {
            break Err("История занята браузером. Повторите чтение.".into());
        }
        if rc != 0 {
            std::thread::sleep(Duration::from_millis(30));
        }
    };
    let finish = unsafe { sqlite3_backup_finish(backup) };
    result?;
    if finish != 0 {
        return Err(snapshot.error());
    }
    snapshot.query(if firefox {
        "SELECT p.url, COALESCE(p.title,''), MAX(v.visit_date)/1000000 FROM moz_historyvisits v JOIN moz_places p ON p.id=v.place_id GROUP BY p.id ORDER BY MAX(v.visit_date) DESC LIMIT 100"
    } else {
        "SELECT u.url, COALESCE(u.title,''), MAX(v.visit_time)/1000000-11644473600 FROM visits v JOIN urls u ON u.id=v.url GROUP BY u.id ORDER BY MAX(v.visit_time) DESC LIMIT 100"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_wal_history_snapshot_both_schemas() {
        let dir = std::env::temp_dir().join(format!("zapret-history-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for firefox in [false, true] {
            let path = dir.join(if firefox { "places.sqlite" } else { "History" });
            let db = Db::open(&path.to_string_lossy(), 6).unwrap();
            db.query("PRAGMA journal_mode=WAL").unwrap();
            let statements = if firefox {
                vec![
                    "CREATE TABLE moz_places(id INTEGER PRIMARY KEY,url TEXT,title TEXT)",
                    "CREATE TABLE moz_historyvisits(place_id INTEGER,visit_date INTEGER)",
                    "INSERT INTO moz_places VALUES(1,'https://example.org/','пример')",
                    "INSERT INTO moz_historyvisits VALUES(1,1700000000000000)",
                ]
            } else {
                vec![
                    "CREATE TABLE urls(id INTEGER PRIMARY KEY,url TEXT,title TEXT)",
                    "CREATE TABLE visits(url INTEGER,visit_time INTEGER)",
                    "INSERT INTO urls VALUES(1,'https://example.org/','пример')",
                    "INSERT INTO visits VALUES(1,13344473600000000)",
                ]
            };
            for sql in statements {
                db.query(sql).unwrap();
            }
            let rows = read(&path, firefox).unwrap();
            assert_eq!(rows[0], ["https://example.org/", "пример", "1700000000"]);
            db.query("BEGIN EXCLUSIVE").unwrap();
            db.query(if firefox {
                "INSERT INTO moz_places VALUES(2,'https://uncommitted.org','hidden')"
            } else {
                "INSERT INTO urls VALUES(2,'https://uncommitted.org','hidden')"
            })
            .unwrap();
            assert_eq!(read(&path, firefox).unwrap(), rows);
            db.query("ROLLBACK").unwrap();
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
