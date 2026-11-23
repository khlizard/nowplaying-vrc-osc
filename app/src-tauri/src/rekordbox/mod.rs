//! rekordbox の master.db を読み取り専用で監視し、再生された曲を取り出す。

pub mod cursor;
pub mod dpapi;
pub mod key_resolver;
pub mod master_db;
pub mod normalize;
pub mod paths;
pub mod source;
pub mod url_key_source;
