//! Types for staff roles and moderation actions.

use std::str::FromStr;

use chrono::{DateTime, Utc};

use derive_more::Display;

use diesel::prelude::*;
use diesel::{delete, insert_into, Insertable, Queryable};

use serde::Serialize;

use super::{PostId, ThreadId};
use crate::schema::{session, staff};
use crate::{Error, Result};

/// A session for a staff member.
#[derive(Debug, Queryable, Insertable)]
#[table_name = "session"]
pub struct Session {
    pub id: String,
    pub expires: DateTime<Utc>,
    pub staff_name: String,
}

/// A staff member.
#[derive(Clone, Debug, Queryable, Insertable, Serialize)]
#[table_name = "staff"]
pub struct Staff {
    pub name: String,
    pub password_hash: String,
    pub role: Role,
}

/// The authority level of a staff member.
#[derive(Clone, Debug, PartialEq, AsExpression, FromSqlRow, Serialize, Display)]
#[sql_type = "sql_types::Role"]
pub enum Role {
    #[display(fmt = "janitor")]
    Janitor,
    #[display(fmt = "moderator")]
    Moderator,
    #[display(fmt = "administrator")]
    Administrator,
}

impl FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "janitor" => Ok(Role::Janitor),
            "moderator" => Ok(Role::Moderator),
            "administrator" => Ok(Role::Administrator),
            _ => Err("Unrecognized variant for enum 'role'".into()),
        }
    }
}

pub mod sql_types {
    //! Boilerplate for dealing with PostgreSQL enums with diesel.

    use std::io::Write;

    use diesel::deserialize::{Result as DeserializeResult, *};
    use diesel::pg::Pg;
    use diesel::serialize::{Result as SerializeResult, *};

    #[derive(SqlType)]
    #[postgres(type_name = "role")]
    pub struct Role;

    impl ToSql<Role, Pg> for super::Role {
        fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> SerializeResult {
            out.write_all(self.to_string().as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSql<Role, Pg> for super::Role {
        fn from_sql(bytes: Option<&[u8]>) -> DeserializeResult<Self> {
            std::str::from_utf8(not_none!(bytes))?
                .parse::<super::Role>()
                .map_err(|err| err.into())
        }
    }
}

pub enum UserAction {
    Ban { user: u8 },
}

pub enum PostAction {
    Delete { posts: Vec<PostId> },
    Move { post: PostId },
    Warn { post: PostId },
}

pub enum ThreadAction {
    Delete { thread: Vec<PostId> },
    Move { thread: PostId },
    Lock { thread: ThreadId },

    Sticky { thread: ThreadId },
    UnSticky { thread: ThreadId },

    Enable { thread: ThreadId },
    Disable { thread: ThreadId },
}

pub enum BoardAction {
    Delete { board: String },
    Create { board: String },
}

impl super::Database {
    /// Get a staff member.
    pub fn staff<S>(&self, name: S) -> Result<Staff>
    where
        S: AsRef<str>,
    {
        use crate::schema::staff::columns::name as column_name;
        use crate::schema::staff::dsl::staff;

        Ok(staff
            .filter(column_name.eq(name.as_ref()))
            .limit(1)
            .first(&self.pool.get()?)?)
    }

    /// Insert a new staff member.
    pub fn insert_staff(&self, new_staff: Staff) -> Result<()> {
        use crate::schema::staff::dsl::staff;

        insert_into(staff)
            .values(&new_staff)
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Delete a staff member.
    pub fn delete_staff<S>(&self, name: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        use crate::schema::staff::columns::name as column_name;
        use crate::schema::staff::dsl::staff;

        delete(staff.filter(column_name.eq(name.as_ref()))).execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Get a session.
    pub fn session<S>(&self, session_id: S) -> Result<Session>
    where
        S: AsRef<str>,
    {
        use crate::schema::session::columns::id;
        use crate::schema::session::dsl::session as session_table;

        let session: Session = session_table
            .filter(id.eq(session_id.as_ref()))
            .limit(1)
            .first(&self.pool.get()?)?;

        if session.expires < Utc::now() {
            self.delete_session(session.staff_name)?;
            return Err(Error::ExpiredSession);
        }

        Ok(session)
    }

    /// Insert a session.
    pub fn insert_session(&self, new_session: Session) -> Result<()> {
        use crate::schema::session::dsl::session;

        insert_into(session)
            .values(&new_session)
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Delete a session.
    pub fn delete_session<S>(&self, session_id: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        use crate::schema::session::columns::id;
        use crate::schema::session::dsl::session;

        delete(session.filter(id.eq(session_id.as_ref()))).execute(&self.pool.get()?)?;

        Ok(())
    }
}
