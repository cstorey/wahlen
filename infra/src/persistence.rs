use std::fmt;

use failure::Error;
use failure::Fail;
use log::*;
use postgres::types::{FromSql, IsNull, ToSql, Type};
use postgres::{accepts, to_sql_checked};
use r2d2_postgres::PostgresConnectionManager;
use serde::{de::DeserializeOwned, Serialize};
use serde_json;

use crate::documents::{HasMeta, Version};
use crate::ids::{Entity, Id};

pub trait Storage {
    fn load<D: DeserializeOwned + Entity>(&self, id: &Id<D>) -> Result<Option<D>, Error>;
    fn save<D: Serialize + Entity + HasMeta>(&self, document: &mut D) -> Result<(), Error>;
}

#[derive(Fail, Debug, PartialEq, Eq)]
#[fail(display = "stale version")]
pub struct ConcurrencyError;

pub struct Documents {
    connection: postgres::Connection,
}

#[derive(Debug)]
pub struct DocumentConnectionManager(PostgresConnectionManager);

struct Jsonb<T>(T);

const SETUP_SQL: &str = include_str!("persistence.sql");
const LOAD_SQL: &str = "SELECT body FROM documents WHERE id = $1";
#[cfg(test)]
const LOAD_NEXT_SQL: &str = "SELECT body
                                     FROM documents
                                     WHERE jsonb_array_length(body -> '_outgoing') > 0
                                     LIMIT 1
";
const INSERT_SQL: &str = "WITH a as (
                                SELECT $1::jsonb as body
                                )
                                INSERT INTO documents AS d (id, body)
                                SELECT a.body ->> '_id', a.body
                                FROM a
                                WHERE NOT EXISTS (
                                    SELECT 1 FROM documents d where d.id = a.body ->> '_id'
                                )";
const UPDATE_SQL: &str = "WITH a as (
                                    SELECT $1::jsonb as body, $2::jsonb as expected_version
                                    )
                                    UPDATE documents AS d
                                        SET body = a.body
                                        FROM a
                                        WHERE id = a.body ->> '_id'
                                        AND d.body -> '_version' = expected_version
                                    ";

impl Documents {
    pub fn setup(&self) -> Result<(), Error> {
        for stmt in SETUP_SQL.split("\n\n") {
            self.connection.batch_execute(stmt)?;
        }
        Ok(())
    }

    pub fn save<D: Serialize + Entity + HasMeta>(&self, document: &mut D) -> Result<(), Error> {
        let t = self.connection.transaction()?;
        let current_version = document.meta().version.clone();

        document.meta_mut().increment_version();

        let rows = if current_version == Version::default() {
            t.prepare_cached(INSERT_SQL)?
                .execute(&[&Jsonb(&document)])?
        } else {
            t.prepare_cached(UPDATE_SQL)?
                .execute(&[&Jsonb(&document), &Jsonb(&current_version)])?
        };
        debug!("Query modified {} rows", rows);
        if rows == 0 {
            return Err(ConcurrencyError.into());
        }
        t.commit()?;

        Ok(())
    }

    pub fn load<D: DeserializeOwned + Entity>(&self, id: &Id<D>) -> Result<Option<D>, Error> {
        let load = self.connection.prepare_cached(LOAD_SQL)?;
        let res = load.query(&[&id.to_string()])?;

        if let Some(row) = res.iter().next() {
            let Jsonb(doc) = row.get(0);

            Ok(Some(doc))
        } else {
            Ok(None)
        }
    }

    #[cfg(test)]
    pub fn load_next_unsent<D: DeserializeOwned + Entity>(&self) -> Result<Option<D>, Error> {
        let load = self.connection.prepare_cached(LOAD_NEXT_SQL)?;
        let res = load.query(&[])?;
        debug!("Cols: {:?}; Rows: {:?}", res.columns(), res.len());

        if let Some(row) = res.iter().next() {
            let Jsonb(doc) = row.get(0);

            Ok(Some(doc))
        } else {
            Ok(None)
        }
    }
}

impl Storage for Documents {
    fn load<D: DeserializeOwned + Entity>(&self, id: &Id<D>) -> Result<Option<D>, Error> {
        Documents::load(self, id)
    }

    fn save<D: Serialize + Entity + HasMeta>(&self, document: &mut D) -> Result<(), Error> {
        Documents::save(self, document)
    }
}

impl DocumentConnectionManager {
    pub fn new(pg: PostgresConnectionManager) -> Self {
        DocumentConnectionManager(pg)
    }
}
impl r2d2::ManageConnection for DocumentConnectionManager {
    type Connection = Documents;
    type Error = postgres::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let connection = self.0.connect()?;
        Ok(Documents { connection })
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        Ok(PostgresConnectionManager::is_valid(
            &self.0,
            &mut conn.connection,
        )?)
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        PostgresConnectionManager::has_broken(&self.0, &mut conn.connection)
    }
}

impl<T: serde::Serialize> ToSql for Jsonb<T> {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut Vec<u8>,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        let val = serde_json::to_value(&self.0)?;
        val.to_sql(ty, out)
    }

    accepts!(postgres::types::JSON, postgres::types::JSONB);

    to_sql_checked!();
}

impl<T: serde::de::DeserializeOwned> FromSql for Jsonb<T> {
    fn from_sql(
        ty: &Type,
        raw: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        let val = serde_json::Value::from_sql(ty, raw)?;
        let actual = serde_json::from_value(val)?;
        Ok(Jsonb(actual))
    }

    accepts!(postgres::types::JSON, postgres::types::JSONB);
}

impl<T: serde::Serialize> fmt::Debug for Jsonb<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Jsonb")
            .field(&serde_json::to_string(&self.0).unwrap_or_else(|_| "???".into()))
            .finish()
    }
}

impl<M> Storage for r2d2::Pool<M>
where
    M: r2d2::ManageConnection,
    M::Connection: Storage,
{
    fn load<D: DeserializeOwned + Entity>(&self, id: &Id<D>) -> Result<Option<D>, Error> {
        let conn = self.get()?;
        conn.load(id)
    }

    fn save<D: Serialize + Entity + HasMeta>(&self, document: &mut D) -> Result<(), Error> {
        let conn = self.get()?;
        conn.save(document)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::documents::*;
    use crate::ids;
    use failure::ResultExt;
    use lazy_static::lazy_static;
    use r2d2::Pool;
    use r2d2_postgres::{PostgresConnectionManager, TlsMode};
    use rand::random;
    use serde::{Deserialize, Serialize};
    use std::env;

    lazy_static! {
        static ref IDGEN: ids::IdGen = ids::IdGen::new();
    }

    #[derive(Debug)]
    struct UseTempSchema(String);

    impl r2d2::CustomizeConnection<Documents, postgres::Error> for UseTempSchema {
        fn on_acquire(&self, conn: &mut Documents) -> Result<(), postgres::Error> {
            loop {
                let t = conn.connection.transaction()?;
                let nschemas: i64 = {
                    let rows = t.query(
                        "SELECT count(*) from pg_catalog.pg_namespace n where n.nspname = $1",
                        &[&self.0],
                    )?;
                    let row = rows.get(0);
                    row.get(0)
                };
                debug!("Number of {} schemas:{}", self.0, nschemas);
                if nschemas == 0 {
                    match t.execute(&format!("CREATE SCHEMA \"{}\"", self.0), &[]) {
                        Ok(_) => {
                            t.commit()?;
                            break;
                        }
                        Err(e) => warn!("Error creating schema:{:?}: {:?}", self.0, e),
                    }
                } else {
                    break;
                }
            }
            conn.connection
                .execute(&format!("SET search_path TO \"{}\"", self.0), &[])?;
            Ok(())
        }
    }

    fn pool(schema: &str) -> Result<Pool<DocumentConnectionManager>, Error> {
        debug!("Build pool for {}", schema);
        let url = env::var("POSTGRES_URL").context("$POSTGRES_URL")?;
        debug!("Use schema name: {}", schema);
        let manager = PostgresConnectionManager::new(&*url, TlsMode::None).expect("postgres");
        let pool = r2d2::Pool::builder()
            .max_size(2)
            .connection_customizer(Box::new(UseTempSchema(schema.to_string())))
            .build(DocumentConnectionManager(manager))?;

        let conn = pool.get()?;
        cleanup(&conn.connection, schema)?;

        debug!("Init schema in {}", schema);
        conn.setup()?;

        Ok(pool)
    }

    fn cleanup(conn: &postgres::Connection, schema: &str) -> Result<(), Error> {
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

    #[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
    struct ADocument {
        #[serde(flatten)]
        meta: DocMeta<ADocument>,
        name: String,
    }

    #[derive(Debug, Clone, Default, Hash, PartialEq, Eq, Deserialize, Serialize)]
    struct AMessage;
    impl Entity for ADocument {
        const PREFIX: &'static str = "adocument";
    }
    impl HasMeta for ADocument {
        fn meta(&self) -> &DocMeta<Self> {
            &self.meta
        }
        fn meta_mut(&mut self) -> &mut DocMeta<Self> {
            &mut self.meta
        }
    }

    #[test]
    fn load_missing_document_should_return_none() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("load_missing_document_should_return_none")?;

        let docs = pool.get()?;

        let loaded = docs.load::<ADocument>(&IDGEN.generate()).expect("load");
        info!("Loaded document: {:?}", loaded);

        assert_eq!(None, loaded);
        Ok(())
    }

    #[test]
    fn save_load() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("save_load")?;
        let some_doc = ADocument {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            name: "Dave".to_string(),
        };

        let docs = pool.get()?;

        info!("Original document: {:?}", some_doc);

        // Ensure we don't accidentally "find" the document by virtue of it
        // being the first in the data file.
        for _ in 0..4 {
            docs.save(&mut ADocument {
                meta: DocMeta::new_with_id(IDGEN.generate()),
                name: format!("{:x}", random::<usize>()),
            })
            .expect("save");
        }
        docs.save(&mut some_doc.clone()).expect("save");
        for _ in 0..4 {
            docs.save(&mut ADocument {
                meta: DocMeta::new_with_id(IDGEN.generate()),
                name: format!("{:x}", random::<usize>()),
            })
            .expect("save");
        }

        let loaded = docs.load(&some_doc.meta.id).expect("load");
        info!("Loaded document: {:?}", loaded);

        assert_eq!(Some(some_doc.name), loaded.map(|d| d.name));
        Ok(())
    }

    #[test]
    fn should_update_on_overwrite() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_update_on_overwrite")?;

        let mut some_doc = ADocument {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            name: "Version 1".to_string(),
        };

        let docs = pool.get()?;

        info!("Original document: {:?}", some_doc);
        docs.save(&mut some_doc).expect("save original");

        let modified_doc = ADocument {
            meta: some_doc.meta.clone(),
            name: "Version 2".to_string(),
        };
        info!("Modified document: {:?}", modified_doc);
        docs.save(&mut modified_doc.clone()).expect("save modified");

        let loaded = docs.load(&some_doc.meta.id).expect("load");
        info!("Loaded document: {:?}", loaded);

        assert_eq!(Some(modified_doc.name), loaded.map(|d| d.name));
        Ok(())
    }

    #[test]
    fn supports_connection() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("supports_connection")?;

        let some_id = IDGEN.generate();

        let docs = pool.get()?;
        docs.save(&mut ADocument {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            name: "Dummy".to_string(),
        })
        .expect("save");
        let _ = docs.load::<ADocument>(&some_id).expect("load");
        Ok(())
    }

    #[test]
    fn should_fail_on_overwrite_with_new() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_fail_on_overwrite_with_new")?;

        let some_doc = ADocument {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            name: "Version 1".to_string(),
        };

        let docs = pool.get()?;

        info!("Original document: {:?}", some_doc);
        docs.save(&mut some_doc.clone()).expect("save original");

        let modified_doc = ADocument {
            meta: DocMeta {
                version: Default::default(),
                ..some_doc.meta
            },
            name: "Version 2".to_string(),
        };

        info!("Modified document: {:?}", modified_doc);
        let err = docs
            .save(&mut modified_doc.clone())
            .expect_err("save should fail");

        info!("Save failed with: {:?}", err);
        info!("root cause: {:?}", err.find_root_cause());
        assert_eq!(
            err.find_root_cause().downcast_ref::<ConcurrencyError>(),
            Some(&ConcurrencyError),
            "Error: {:?}",
            err
        );
        Ok(())
    }

    #[test]
    fn should_fail_on_overwrite_with_bogus_version() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_fail_on_overwrite_with_bogus_version")?;

        let docs = pool.get()?;

        let id = IDGEN.generate();
        let mut some_doc = ADocument {
            meta: DocMeta::new_with_id(id),
            name: "Version 1".to_string(),
        };

        info!("Original document: {:?}", some_doc);
        docs.save(&mut some_doc)?;

        let mut old_doc = ADocument {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            name: "Old".to_string(),
        };
        for _ in 0..4 {
            docs.save(&mut old_doc)?;
        }
        debug!("Old document: {:?}", old_doc);

        assert_ne!(some_doc.meta.version, old_doc.meta.version);

        some_doc.meta.version = old_doc.meta.version;

        info!("Modified document: {:?}", some_doc);
        let err = docs
            .save(&mut some_doc.clone())
            .expect_err("save should fail");

        assert_eq!(
            err.find_root_cause().downcast_ref::<ConcurrencyError>(),
            Some(&ConcurrencyError),
            "Error: {:?}",
            err
        );
        Ok(())
    }

    #[test]
    fn should_fail_on_new_document_with_nonzero_version() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_fail_on_new_document_with_nonzero_version")?;
        let docs = pool.get()?;

        let mut old_doc = ADocument {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            name: "Old".to_string(),
        };
        for _ in 0..4 {
            docs.save(&mut old_doc)?;
        }
        debug!("Old document: {:?}", old_doc);

        let mut meta = DocMeta::new_with_id(IDGEN.generate());
        meta.version = old_doc.meta.version;
        let name = "Version 1".to_string();
        let some_doc = ADocument { meta, name };

        info!("new misAsRef<DocMeta> document: {:?}", some_doc);
        let err = docs
            .save(&mut some_doc.clone())
            .expect_err("save should fail");

        assert_eq!(
            err.find_root_cause().downcast_ref::<ConcurrencyError>(),
            Some(&ConcurrencyError),
            "Error: {:?}",
            err
        );
        Ok(())
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    struct ChattyDoc {
        #[serde(flatten)]
        meta: DocMeta<ChattyDoc>,
        #[serde(flatten)]
        mbox: MailBox<AMessage>,
    }

    impl Entity for ChattyDoc {
        const PREFIX: &'static str = "chatty";
    }
    impl HasMeta for ChattyDoc {
        fn meta(&self) -> &DocMeta<Self> {
            &self.meta
        }
        fn meta_mut(&mut self) -> &mut DocMeta<Self> {
            &mut self.meta
        }
    }

    #[test]
    fn should_enqueue_nothing_by_default() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_enqueue_nothing_by_default")?;
        let docs = pool.get()?;

        let mut some_doc = ChattyDoc {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            mbox: MailBox::default(),
        };

        info!("Original document: {:?}", some_doc);

        docs.save(&mut some_doc).expect("save");

        let docp = docs.load_next_unsent::<ChattyDoc>()?;
        info!("Loaded something: {:?}", docp);

        assert!(docp.is_none(), "Should find no document. Got: {:?}", docp);
        Ok(())
    }

    #[test]
    fn should_enqueue_on_create() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_enqueue_on_create")?;
        let docs = pool.get()?;

        let mut some_doc = ChattyDoc {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            mbox: MailBox::default(),
        };

        some_doc.mbox.send(AMessage);
        info!("Original document: {:?}", some_doc);
        docs.save(&mut some_doc).expect("save");

        let docp = docs.load_next_unsent::<ChattyDoc>()?;
        info!("Loaded something: {:?}", docp);

        let loaded = docs.load_next_unsent::<ChattyDoc>()?;
        info!("Loaded something: {:?}", loaded);

        assert_eq!(Some(some_doc.meta.id), loaded.map(|d| d.meta.id));

        Ok(())
    }

    #[test]
    fn should_enqueue_on_update() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_enqueue_on_update")?;
        let docs = pool.get()?;

        let mut some_doc = ChattyDoc {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            mbox: MailBox::default(),
        };

        docs.save(&mut some_doc)?;

        some_doc.mbox.send(AMessage);
        info!("Original document: {:?}", some_doc);
        docs.save(&mut some_doc).expect("save");

        let loaded = docs.load_next_unsent::<ChattyDoc>()?;
        info!("Loaded something: {:?}", loaded);

        assert_eq!(Some(some_doc.meta.id), loaded.map(|d| d.meta.id));
        Ok(())
    }

    #[test]
    #[ignore]
    fn should_enqueue_something_something() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("should_enqueue_something_something")?;

        let mut some_doc = ChattyDoc {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            mbox: MailBox::default(),
        };
        some_doc.mbox.send(AMessage);

        let docs = pool.get()?;
        info!("Original document: {:?}", some_doc);

        docs.save(&mut some_doc)?;

        let doc = docs
            .load_next_unsent::<ChattyDoc>()?
            .ok_or_else(|| failure::err_msg("missing document?"))?;;
        info!("Loaded something: {:?}", doc);

        assert_eq!(doc.meta.id, some_doc.meta.id);

        Ok(())
    }

    #[test]
    fn save_load_via_pool() -> Result<(), Error> {
        env_logger::try_init().unwrap_or_default();
        let pool = pool("save_load_via_pool")?;
        let some_doc = ADocument {
            meta: DocMeta::new_with_id(IDGEN.generate()),
            name: "Dave".to_string(),
        };

        info!("Original document: {:?}", some_doc);

        pool.save(&mut some_doc.clone()).expect("save");
        let loaded = pool.load(&some_doc.meta.id).expect("load");
        info!("Loaded document: {:?}", loaded);

        assert_eq!(Some(some_doc.name), loaded.map(|d| d.name));
        Ok(())
    }

    #[test]
    #[ignore]
    fn should_only_load_messages_of_type() {}
}
