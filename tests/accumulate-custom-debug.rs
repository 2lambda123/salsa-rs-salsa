mod common;

use expect_test::expect;
use salsa::{Accumulator, Database};
use test_log::test;

#[salsa::input]
struct MyInput {
    count: u32,
}

#[salsa::accumulator(no_debug)]
struct Log(String);

impl std::fmt::Debug for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CustomLog").field(&self.0).finish()
    }
}

#[salsa::tracked]
fn push_logs(db: &dyn salsa::Database, input: MyInput) -> salsa::Result<()> {
    for i in 0..input.count(db)? {
        Log(format!("#{i}")).accumulate(db);
    }

    Ok(())
}

#[test]
fn accumulate_custom_debug() -> salsa::Result<()> {
    salsa::DatabaseImpl::new().attach(|db| {
        let input = MyInput::new(db, 2);
        let logs = push_logs::accumulated::<Log>(db, input)?;
        expect![[r##"
            [
                CustomLog(
                    "#0",
                ),
                CustomLog(
                    "#1",
                ),
            ]
        "##]]
        .assert_debug_eq(&logs);

        Ok(())
    })
}
