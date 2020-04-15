//! Types for staff roles and moderation actions.

use std::net::IpAddr;
use std::str::FromStr;
use std::convert::TryInto;

use argon2::hash_encoded;

use chrono::{DateTime, Duration, Utc};

use derive_more::Display;

use diesel::prelude::*;
use diesel::{delete, insert_into, update, Insertable, Queryable};

use serde::Serialize;

use super::{PostId, ThreadId};
use crate::schema::{anon_user, session, staff};
use crate::{Database, Error, Result};

pub type UserId = i32;

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
#[derive(
    Clone, Debug, PartialEq, AsExpression, FromSqlRow, Serialize, Display,
)]
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

/// An anonymous site user.
#[derive(Debug, Queryable, Serialize)]
pub struct User {
    /// The user's ID in the database.
    pub id: UserId,
    /// The hash of the user's IP address.
    pub hash: String,
    /// If the user is banned, when the user's ban expires.
    pub ban_expires: Option<DateTime<Utc>>,
    /// A note about a user by a moderator.
    pub note: Option<String>,
    /// The user's IP address.
    pub ip: String,
}

impl User {
    /// Whether or not the user is banned.
    pub fn is_banned(&self) -> bool {
        self.ban_expires
            .map(|time| time > Utc::now())
            .unwrap_or(false)
    }
}

/// A new anonymous site user to insert into the database.
#[derive(Debug, Insertable)]
#[table_name = "anon_user"]
pub struct NewUser {
    pub hash: String,
    pub ban_expires: Option<DateTime<Utc>>,
    pub note: Option<String>,
    pub ip: String,
}

impl NewUser {
    /// Create a `NewUser` from an user's IP address.
    pub fn from_ip(ip: IpAddr) -> NewUser {
        NewUser {
            hash: NewUser::hash_ip(ip),
            ban_expires: None,
            note: None,
            ip: ip.to_string(),
        }
    }

    /// The configuration to use for IP hashing.
    fn ip_hash_config() -> argon2::Config<'static> {
        argon2::Config {
            mem_cost: 1024,
            time_cost: 1,
            ..argon2::Config::default()
        }
    }

    /// Hash a user's IP address.
    pub fn hash_ip(ip: IpAddr) -> String {
        let salt = b"longboard-user";
        let conf = Self::ip_hash_config();

        let octets = match ip {
            IpAddr::V4(v4_addr) => v4_addr.octets().to_vec(),
            IpAddr::V6(v6_addr) => v6_addr.octets().to_vec(),
        };

        hash_encoded(&octets, salt, &conf)
            .expect("could not hash IP address with Argon2")
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

impl Database {
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
    pub fn insert_staff(&self, new_staff: &Staff) -> Result<()> {
        use crate::schema::staff::dsl::staff;

        insert_into(staff)
            .values(new_staff)
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

        delete(staff.filter(column_name.eq(name.as_ref())))
            .execute(&self.pool.get()?)?;

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
    pub fn insert_session(&self, new_session: &Session) -> Result<()> {
        use crate::schema::session::dsl::session;

        insert_into(session)
            .values(new_session)
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

        delete(session.filter(id.eq(session_id.as_ref())))
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Get a user by their IP.
    pub fn user(&self, user_ip: IpAddr) -> Result<User> {
        // It's more efficient to get every user from the database and then
        // check each of them then it is to compute the hash of the IP address
        // and then select only that user from the database.
        //
        // This is because it's much cheaper to verify an Argon2 hash than it
        // is to compute an Argon2 hash.

        use crate::schema::anon_user::columns::ip;
        use crate::schema::anon_user::dsl::anon_user;

        Ok(anon_user
            .filter(ip.eq(user_ip.to_string()))
            .limit(1)
            .first(&self.pool.get()?)?)
    }

    /// Get all users.
    pub fn all_users(&self) -> Result<Vec<User>> {
        use crate::schema::anon_user::dsl::anon_user;

        Ok(anon_user.load(&self.pool.get()?)?)
    }

    /// Get the total number of posts a user has made.
    pub fn user_post_count(&self, user_id: UserId) -> Result<u32> {
        use crate::schema::post::dsl::post;
        use crate::schema::post::columns::user_id as column_user_id;

        let count: i64 = post
           .filter(column_user_id.eq(user_id))
           .count()
           .first(&self.pool.get()?)?;

        Ok(count.try_into().unwrap())
    }

    /// Insert a user.
    pub fn insert_user(&self, new_user: &NewUser) -> Result<User> {
        use crate::schema::anon_user::dsl::anon_user;

        let user = insert_into(anon_user)
            .values(new_user)
            .get_result(&self.pool.get()?)?;

        Ok(user)
    }

    /// Ban a user.
    pub fn ban_user(
        &self,
        user_id: UserId,
        ban_duration: Duration,
    ) -> Result<()> {
        use crate::schema::anon_user::columns::{ban_expires, id};
        use crate::schema::anon_user::dsl::anon_user;

        update(anon_user.filter(id.eq(user_id)))
            .set(ban_expires.eq(Some(Utc::now() + ban_duration)))
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Unban a user.
    pub fn unban_user(&self, user_id: UserId) -> Result<()> {
        use crate::schema::anon_user::columns::{ban_expires, id};
        use crate::schema::anon_user::dsl::anon_user;

        update(anon_user.filter(id.eq(user_id)))
            .set(ban_expires.eq::<Option<DateTime<Utc>>>(None))
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Update the moderation notes for a user.
    pub fn set_user_note<S>(&self, user_id: UserId, new_note: S) -> Result<()>
    where
        S: Into<String>,
    {
        use crate::schema::anon_user::columns::{id, note};
        use crate::schema::anon_user::dsl::anon_user;

        update(anon_user.filter(id.eq(user_id)))
            .set(note.eq(Some(new_note.into())))
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Remove the moderation notes for a user.
    pub fn remove_user_note(&self, user_id: UserId) -> Result<()> {
        use crate::schema::anon_user::columns::{id, note};
        use crate::schema::anon_user::dsl::anon_user;

        update(anon_user.filter(id.eq(user_id)))
            .set(note.eq::<Option<String>>(None))
            .execute(&self.pool.get()?)?;

        Ok(())
    }

    /// Delete all of the posts a user has made.
    pub fn delete_posts_for_user(&self, id: UserId) -> Result<()> {
        use crate::schema::post::columns::user_id;
        use crate::schema::post::dsl::post;

        delete(post.filter(user_id.eq(id))).execute(&self.pool.get()?)?;

        Ok(())
    }
}
