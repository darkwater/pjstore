#![feature(proc_macro_hygiene, decl_macro)]

use rocket::{delete, get, post, put, routes};
use rocket_contrib::json::{Json, JsonValue};
use std::collections::HashMap;
use std::default::Default;
use structopt::StructOpt;

#[rocket_contrib::database("primary")]
struct DbConn(rusqlite::Connection);

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Arguments {
    /// Path to SQLite database
    database: String,
}

fn merge_json(a: &mut serde_json::Value, b: serde_json::Value) {
    if let serde_json::Value::Object(a) = a {
        if let serde_json::Value::Object(b) = b {
            for (k, v) in b {
                if v.is_null() {
                    a.remove(&k);
                }
                else {
                    merge_json(a.entry(k).or_insert(serde_json::Value::Null), v);
                }
            } 

            return;
        }
    }

    *a = b;
}

#[get("/<key>/<name>")]
fn get_document(key: String, name: String, conn: DbConn) -> String {
    let res = conn.query_row(
        "SELECT data FROM documents WHERE key = ?1 AND name = ?2",
        &[&key, &name],
        |row| row.get(0),
    );

    match res {
        Err(rusqlite::Error::QueryReturnedNoRows) => format!("key or name not found"),
        Err(error) => format!("error: {}", dbg!(error)),
        Ok(data) => data,
    }
}

#[put("/<key>/<name>", data = "<data>")]
fn put_document(key: String, name: String, data: Json<JsonValue>, conn: DbConn) -> String {
    let res = conn.query_row("SELECT key FROM keys WHERE key = ?1", &[&key], |_| ());

    match res {
        Err(rusqlite::Error::QueryReturnedNoRows) => return format!("key not found"),
        Err(error) => return format!("error: {}", dbg!(error)),
        Ok(()) => (), // continue
    }

    let res = conn.execute(
        "INSERT INTO documents VALUES (NULL, ?1, ?2, ?3)",
        &[&key, &name, &serde_json::to_string(&data.into_inner()).unwrap()],
    );

    match res {
        Err(error) => format!("error: {}", dbg!(error)),
        Ok(1) => format!("ok"),
        Ok(n) => format!("unexpected result: {}", n),
    }
}

#[post("/<key>/<name>", data = "<newdata>")]
fn post_document(key: String, name: String, newdata: Json<JsonValue>, conn: DbConn) -> String {
    let res: Result<String, _> = conn.query_row(
        "SELECT data FROM documents WHERE key = ?1 AND name = ?2",
        &[&key, &name],
        |row| row.get(0),
    );

    let mut data = match res {
        Err(rusqlite::Error::QueryReturnedNoRows) => serde_json::Value::Object(Default::default()),
        Err(error) => return format!("error: {}", dbg!(error)),
        Ok(data) => serde_json::from_str(&data).unwrap(),
    };

    merge_json(&mut data, newdata.into_inner().into());

    let res = conn.execute(
        "INSERT INTO documents VALUES (NULL, ?1, ?2, ?3)",
        &[&key, &name, &serde_json::to_string(&data).unwrap()],
    );

    match res {
        Err(error) => format!("error: {}", dbg!(error)),
        Ok(1) => format!("ok"),
        Ok(n) => format!("unexpected result: {}", n),
    }
}

#[delete("/<key>/<name>")]
fn delete_document(key: String, name: String, conn: DbConn) -> String {
    let res = conn.execute("DELETE FROM documents WHERE key = ?1 AND name = ?2", &[&key, &name]);

    match res {
        Err(error) => format!("error: {}", dbg!(error)),
        Ok(0) => format!("document or key not found"),
        Ok(1) => format!("ok"),
        Ok(n) => format!("unexpected result: {}", n),
    }
}

fn main() {
    let args: Arguments = StructOpt::from_args();

    {
        let conn = rusqlite::Connection::open(&args.database).unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS keys (
                id      INTEGER PRIMARY KEY,
                key     TEXT NOT NULL UNIQUE
            )",
            &[],
            ).unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS documents (
                id      INTEGER PRIMARY KEY,
                key     TEXT NOT NULL,
                name    TEXT NOT NULL,
                data    TEXT NOT NULL,

                CONSTRAINT keyname UNIQUE (key, name) ON CONFLICT REPLACE
            )",
            &[],
            ).unwrap();
    }

    let mut database_config = HashMap::new();
    let mut databases = HashMap::new();
    database_config.insert("url", rocket::config::Value::from(args.database));
    databases.insert("primary", rocket::config::Value::from(database_config));

    let config = rocket::config::Config::build(rocket::config::Environment::Development)
        .extra("databases", databases)
        .finalize()
        .unwrap();

    rocket::custom(config)
        .attach(DbConn::fairing())
        .mount("/", routes![
            get_document, put_document, post_document, delete_document,
        ])
        .launch();
}
