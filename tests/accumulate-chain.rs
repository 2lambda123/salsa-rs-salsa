//! Test that when having nested tracked functions
//! we don't drop any values when accumulating.

mod common;

use expect_test::expect;
use salsa::{Accumulator, Database, DatabaseImpl};
use test_log::test;

#[salsa::accumulator]
struct Log(#[allow(dead_code)] String);

#[salsa::tracked]
fn push_logs(db: &dyn Database) -> salsa::Result<()> {
    push_a_logs(db)?;
    Ok(())
}

#[salsa::tracked]
fn push_a_logs(db: &dyn Database) -> salsa::Result<()> {
    Log("log a".to_string()).accumulate(db);
    push_b_logs(db)?;

    Ok(())
}

#[salsa::tracked]
fn push_b_logs(db: &dyn Database) -> salsa::Result<()> {
    // No logs
    push_c_logs(db)?;
    Ok(())
}

#[salsa::tracked]
fn push_c_logs(db: &dyn Database) -> salsa::Result<()> {
    // No logs
    push_d_logs(db)?;
    Ok(())
}

#[salsa::tracked]
fn push_d_logs(db: &dyn Database) -> salsa::Result<()> {
    Log("log d".to_string()).accumulate(db);
    Ok(())
}

#[test]
fn accumulate_chain() -> salsa::Result<()> {
    DatabaseImpl::new().attach(|db| {
        let logs = push_logs::accumulated::<Log>(db)?;
        // Check that we get all the logs.
        expect![[r#"
            [
                Log(
                    "log a",
                ),
                Log(
                    "log d",
                ),
            ]"#]]
        .assert_eq(&format!("{:#?}", logs));

        Ok(())
    })
}
