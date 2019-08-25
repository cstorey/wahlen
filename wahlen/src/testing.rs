#![cfg(test)]
use failure::{Fallible, ResultExt};
use infra::persistence::*;
use postgres;
use r2d2::Pool;
use r2d2_postgres::{PostgresConnectionManager, TlsMode};
use std::env;

use log::*;

pub fn pool(schema: &str) -> Fallible<Pool<DocumentConnectionManager>> {
    debug!("Build pool for {}", schema);
    let url = env::var("POSTGRES_URL").context("$POSTGRES_URL")?;
    debug!("Use schema name: {}", schema);
    let manager = PostgresConnectionManager::new(&*url, TlsMode::None).expect("postgres");
    let pool = r2d2::Pool::builder()
        .max_size(2)
        .connection_customizer(Box::new(UseSchema(schema.to_string())))
        .build(DocumentConnectionManager::new(manager))?;

    let conn = pool.get()?;
    cleanup(&conn.get_ref(), schema)?;

    debug!("Init schema in {}", schema);
    conn.setup()?;

    Ok(pool)
}

fn cleanup(conn: &postgres::Connection, schema: &str) -> Fallible<()> {
    let t = conn.transaction()?;
    debug!("Clean old tables in {}", schema);
    for row in t
        .query(
            "SELECT n.nspname, c.relname \
             FROM pg_catalog.pg_class c \
             LEFT JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 and c.relkind = 'r'",
            &[&schema],
        )?
        .iter()
    {
        let schema = row.get::<_, String>(0);
        let table = row.get::<_, String>(1);
        t.execute(&format!("DROP TABLE {}.{}", schema, table), &[])?;
    }
    t.commit()?;
    Ok(())
}
