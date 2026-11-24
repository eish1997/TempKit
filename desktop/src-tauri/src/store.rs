//! SQLite metadata boundary. Call from the host worker, never the hit-test loop.
use rusqlite::{Connection, TransactionBehavior};
use std::path::Path;

pub const SCHEMA_VERSION: i64 = 15;
pub struct Store { pub(crate) connection: Connection }

impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut connection = Connection::open(path).map_err(|e| e.to_string())?;
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e| e.to_string())?;
        // Inspect before changing pragmas or schema: unknown/corrupt files stay intact.
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(|e| e.to_string())?;
        if version > SCHEMA_VERSION { return Err(format!("unsupported schema version {version}")); }
        let integrity: String = connection.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if integrity != "ok" { return Err(format!("database integrity: {integrity}")); }
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;").map_err(|e| e.to_string())?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
        let locked_version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(|e| e.to_string())?;
        if locked_version == 0 {
            tx.execute_batch(include_str!("store_v1.sql")).map_err(|e| e.to_string())?;
        } else if locked_version > SCHEMA_VERSION { return Err("schema changed while opening".into()); }
        if locked_version < 2 {tx.execute_batch(include_str!("store_v2.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 3 {tx.execute_batch(include_str!("store_v3.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 4 {tx.execute_batch(include_str!("store_v4.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 5 {tx.execute_batch(include_str!("store_v5.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 6 {tx.execute_batch(include_str!("store_v6.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 7 {tx.execute_batch(include_str!("store_v7.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 8 {tx.execute_batch(include_str!("store_v8.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 9 {tx.execute_batch(include_str!("store_v9.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 10 {tx.execute_batch(include_str!("store_v10.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 11 {tx.execute_batch(include_str!("store_v11.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 12 {tx.execute_batch(include_str!("store_v12.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 13 {tx.execute_batch(include_str!("store_v13.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 14 {tx.execute_batch(include_str!("store_v14.sql")).map_err(|e|e.to_string())?;}
        if locked_version < 15 {crate::notes::migrate_v15(&tx)?;}
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Self { connection })
    }
    pub fn schema_version(&self) -> Result<i64, String> {
        self.connection.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn v13_cost_scope_upgrade_is_atomic(){
        for fail in [false,true]{
            let p=path();let s=Store::open(&p).unwrap();
            s.connection.execute_batch("DROP TABLE image_cost_scopes; DROP TRIGGER note_tag_birth_no_update; DROP TABLE note_tag_births; DELETE FROM schema_migrations WHERE version>=14; PRAGMA user_version=13; INSERT INTO image_budget VALUES(1,2000000);").unwrap();
            if fail{s.connection.execute_batch("CREATE TABLE image_cost_scopes(original TEXT); INSERT INTO image_cost_scopes VALUES('preserved');").unwrap();}drop(s);
            let result=Store::open(&p);
            if fail{assert!(result.is_err());}else{assert_eq!(result.unwrap().schema_version().unwrap(),SCHEMA_VERSION);}
            let c=Connection::open(&p).unwrap();assert_eq!(c.query_row("SELECT limit_micros FROM image_budget",[],|r|r.get::<_,i64>(0)).unwrap(),2000000);
            if fail{assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),13);assert_eq!(c.query_row("SELECT original FROM image_cost_scopes",[],|r|r.get::<_,String>(0)).unwrap(),"preserved");}
        }
    }
    #[test]fn v12_rotation_upgrade_preserves_budget_and_rolls_back_failure(){
        for fail in [false,true]{
            let p=path();let c=Connection::open(&p).unwrap();
            for sql in [include_str!("store_v1.sql"),include_str!("store_v2.sql"),include_str!("store_v3.sql"),include_str!("store_v4.sql"),include_str!("store_v5.sql"),include_str!("store_v6.sql"),include_str!("store_v7.sql"),include_str!("store_v8.sql"),include_str!("store_v9.sql"),include_str!("store_v10.sql"),include_str!("store_v11.sql"),include_str!("store_v12.sql")]{c.execute_batch(sql).unwrap();}
            c.execute("INSERT INTO image_budget VALUES(1,2000000)",[]).unwrap();c.execute("INSERT INTO scenes VALUES('main',77,'preserved')",[]).unwrap();
            if fail{c.execute_batch("CREATE TABLE gateway_credential_rotations(original TEXT); INSERT INTO gateway_credential_rotations VALUES('keep');").unwrap();}drop(c);
            let result=Store::open(&p);if fail{assert!(result.is_err())}else{assert_eq!(result.unwrap().schema_version().unwrap(),SCHEMA_VERSION)}
            let c=Connection::open(&p).unwrap();assert_eq!(c.query_row("SELECT limit_micros FROM image_budget",[],|r|r.get::<_,i64>(0)).unwrap(),2000000);assert_eq!(c.query_row("SELECT scene_json FROM scenes",[],|r|r.get::<_,String>(0)).unwrap(),"preserved");
            if fail{assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),12);assert_eq!(c.query_row("SELECT original FROM gateway_credential_rotations",[],|r|r.get::<_,String>(0)).unwrap(),"keep");}
        }
    }
    #[test]fn v11_release_upgrade_preserves_budget_and_failed_upgrade_is_atomic(){
        for fail in [false,true]{
            let p=path();let c=Connection::open(&p).unwrap();
            for sql in [include_str!("store_v1.sql"),include_str!("store_v2.sql"),include_str!("store_v3.sql"),include_str!("store_v4.sql"),include_str!("store_v5.sql"),include_str!("store_v6.sql"),include_str!("store_v7.sql"),include_str!("store_v8.sql"),include_str!("store_v9.sql"),include_str!("store_v10.sql"),include_str!("store_v11.sql")]{c.execute_batch(sql).unwrap();}
            c.execute("INSERT INTO image_budget VALUES(1,2000000)",[]).unwrap();
            if fail{c.execute_batch("CREATE TABLE image_budget_releases(original TEXT); INSERT INTO image_budget_releases VALUES('keep');").unwrap();}drop(c);
            let result=Store::open(&p);if fail{assert!(result.is_err())}else{assert_eq!(result.unwrap().schema_version().unwrap(),SCHEMA_VERSION)}
            let c=Connection::open(&p).unwrap();assert_eq!(c.query_row("SELECT limit_micros FROM image_budget",[],|r|r.get::<_,i64>(0)).unwrap(),2000000);
            if fail{assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),11);assert_eq!(c.query_row("SELECT original FROM image_budget_releases",[],|r|r.get::<_,String>(0)).unwrap(),"keep");}
        }
    }
    #[test]fn v10_budget_upgrade_preserves_data_and_rolls_back_failure(){
        for fail in [false,true]{
            let p=path();let c=Connection::open(&p).unwrap();
            for sql in [include_str!("store_v1.sql"),include_str!("store_v2.sql"),include_str!("store_v3.sql"),include_str!("store_v4.sql"),include_str!("store_v5.sql"),include_str!("store_v6.sql"),include_str!("store_v7.sql"),include_str!("store_v8.sql"),include_str!("store_v9.sql"),include_str!("store_v10.sql")]{c.execute_batch(sql).unwrap();}
            c.execute("INSERT INTO scenes VALUES('main',77,'preserved')",[]).unwrap();
            if fail{c.execute_batch("CREATE TABLE image_budget_reservations(original TEXT); INSERT INTO image_budget_reservations VALUES('keep');").unwrap();}drop(c);
            let result=Store::open(&p);if fail{assert!(result.is_err())}else{assert_eq!(result.unwrap().schema_version().unwrap(),SCHEMA_VERSION)}
            let c=Connection::open(&p).unwrap();assert_eq!(c.query_row("SELECT scene_json FROM scenes",[],|r|r.get::<_,String>(0)).unwrap(),"preserved");
            if fail{assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),10);assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE name='image_budget'",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT original FROM image_budget_reservations",[],|r|r.get::<_,String>(0)).unwrap(),"keep");}
        }
    }
    #[test]fn v6_notes_upgrade_preserves_scene_and_failed_upgrade_rolls_back(){
        for fail in [false,true]{
            let p=path();
            let c=Connection::open(&p).unwrap();
            for sql in [include_str!("store_v1.sql"),include_str!("store_v2.sql"),include_str!("store_v3.sql"),include_str!("store_v4.sql"),include_str!("store_v5.sql"),include_str!("store_v6.sql")]{c.execute_batch(sql).unwrap();}
            c.execute("INSERT INTO scenes VALUES('main',42,'unchanged')",[]).unwrap();
            if fail{c.execute_batch("CREATE TABLE notes(original TEXT); INSERT INTO notes VALUES('preserve');").unwrap();}
            drop(c);
            let result=Store::open(&p);
            if fail{assert!(result.is_err());}else{assert_eq!(result.unwrap().schema_version().unwrap(),SCHEMA_VERSION);}
            let c=Connection::open(&p).unwrap();
            assert_eq!(c.query_row("SELECT scene_json FROM scenes",[],|r|r.get::<_,String>(0)).unwrap(),"unchanged");
            if fail{
                assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),6);
                assert_eq!(c.query_row("SELECT original FROM notes",[],|r|r.get::<_,String>(0)).unwrap(),"preserve");
            }
        }
    }
    fn path() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!("quickgen-store-{}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap(); dir.join("metadata.sqlite")
    }
    #[test] fn initialize_reopen_and_constraints() {
        let p = path();
        { let s = Store::open(&p).unwrap(); assert_eq!(s.schema_version().unwrap(), SCHEMA_VERSION);
          assert!(s.connection.execute("INSERT INTO resource_refs VALUES ('task','missing','absent')", []).is_err()); }
        assert_eq!(Store::open(&p).unwrap().schema_version().unwrap(), SCHEMA_VERSION);
    }
    #[test] fn upgrade_preserves_existing_data() {
        let p = path();
        { let c = Connection::open(&p).unwrap(); c.execute_batch("CREATE TABLE legacy(value TEXT); INSERT INTO legacy VALUES ('keep');").unwrap(); }
        let s = Store::open(&p).unwrap();
        assert_eq!(s.connection.query_row("SELECT value FROM legacy", [], |r| r.get::<_,String>(0)).unwrap(), "keep");
    }
    #[test] fn v1_upgrade_keeps_resource_records() {
        let p=path();{let c=Connection::open(&p).unwrap();c.execute_batch(include_str!("store_v1.sql")).unwrap();c.execute_batch("INSERT INTO blobs VALUES('b','digest',1); INSERT INTO resources VALUES('r','b','{}','available');").unwrap();}
        let s=Store::open(&p).unwrap();assert_eq!(s.schema_version().unwrap(),SCHEMA_VERSION);
        assert_eq!(s.connection.query_row("SELECT resource_id FROM resources",[],|r|r.get::<_,String>(0)).unwrap(),"r");
    }
    #[test] fn failed_upgrade_rolls_back_all_schema_changes() {
        let p = path();
        { let c = Connection::open(&p).unwrap(); c.execute_batch("CREATE TABLE resources(original TEXT); INSERT INTO resources VALUES ('keep');").unwrap(); }
        assert!(Store::open(&p).is_err());
        let c = Connection::open(&p).unwrap();
        assert_eq!(c.query_row("PRAGMA user_version", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE name='schema_migrations'", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(c.query_row("SELECT original FROM resources", [], |r| r.get::<_,String>(0)).unwrap(), "keep");
    }
    #[test] fn future_and_corrupt_database_preserved() {
        for corrupt in [false,true] {
            let p = path();
            if corrupt { std::fs::write(&p, b"not a sqlite database").unwrap(); }
            else { Connection::open(&p).unwrap().execute_batch("PRAGMA user_version=999;").unwrap(); }
            let before = std::fs::read(&p).unwrap(); assert!(Store::open(&p).is_err());
            assert_eq!(std::fs::read(&p).unwrap(), before);
        }
    }
    // Spawn the same test executable, then exit without dropping the transaction.
    // This exercises SQLite hot-journal recovery, not only Rust rollback-on-drop.
    #[test] fn abrupt_transaction_exit_recovers() {
        const KEY: &str = "QUICKGEN_STORE_CRASH_TEST_DB";
        if let Some(p) = std::env::var_os(KEY) {
            let mut s = Store::open(Path::new(&p)).unwrap();
            s.connection.execute_batch("PRAGMA cache_size=1;").unwrap();
            let tx = s.connection.transaction().unwrap();
            tx.execute_batch("CREATE TABLE interrupted_write(value BLOB); INSERT INTO interrupted_write VALUES(zeroblob(2097152));").unwrap();
            std::process::exit(73);
        }
        let p = path();
        { let s = Store::open(&p).unwrap(); s.connection.execute_batch("CREATE TABLE survivor(value TEXT); INSERT INTO survivor VALUES ('saved');").unwrap(); }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "store::tests::abrupt_transaction_exit_recovers", "--nocapture"])
            .env(KEY, &p).status().unwrap();
        assert_eq!(status.code(), Some(73));
        let s = Store::open(&p).unwrap();
        assert_eq!(s.connection.query_row("SELECT value FROM survivor", [], |r| r.get::<_,String>(0)).unwrap(), "saved");
        assert_eq!(s.connection.query_row("SELECT count(*) FROM sqlite_master WHERE name='interrupted_write'", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    }
}
