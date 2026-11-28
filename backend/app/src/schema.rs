// ── Diesel table definitions (type-safe queries) ──
//
// Tables are in the `auth` PostgreSQL schema.  The table name `keys`
// resolves to `auth.keys` via search_path set at connection time
// (see db.rs).

pub(crate) mod auth {
    diesel::table! {
        keys (key_id) {
            id              -> diesel::sql_types::Uuid,
            key_id          -> diesel::sql_types::Text,
            label           -> diesel::sql_types::Text,
            pub_key         -> diesel::sql_types::Binary,
            capabilities    -> diesel::sql_types::SmallInt,
            is_active       -> diesel::sql_types::Bool,
            nonce_counter   -> diesel::sql_types::BigInt,
            created_at      -> diesel::sql_types::Timestamptz,
        }
    }

}
