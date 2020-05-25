//! Types for staff roles and moderation actions.

use std::convert::TryInto;
use std::net::IpAddr;
use std::str::FromStr;

use argon2::hash_encoded;

use chrono::{DateTime, Duration, Utc};

use derive_more::Display;

use diesel::prelude::*;
use diesel::{delete, insert_into, update, Insertable, Queryable};

use serde::Serialize;

use crate::models::{Connection, InnerConnection};
use crate::schema::{anon_user, session, staff, staff_action};
use crate::{Error, Result};

/// A session for a staff member.
#[derive(Debug)]
pub struct Session {
    pub id: String,
    pub expires: DateTime<Utc>,
    pub staff: Staff,
}

/// The database model for a session.
#[derive(Debug, Queryable, Insertable)]
#[table_name = "session"]
struct DbSession {
    id: String,
    expires: DateTime<Utc>,
    staff_name: String,
}

/// A staff member.
#[derive(Clone, Debug, Queryable, Insertable, Serialize)]
#[table_name = "staff"]
pub struct Staff {
    pub name: String,
    pub password_hash: String,
    pub role: Role,
}

impl Staff {
    /// Whether or not a staff member is authorized for a role.
    pub fn is_authorized(&self, role: Role) -> bool {
        self.role >= role
    }
}

/// The authority level of a staff member.
#[derive(
    Copy,
    Clone,
    Debug,
    Display,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsExpression,
    FromSqlRow,
    Serialize,
)]
#[sql_type = "sql_types::Role"]
pub enum Role {
    Janitor,
    Moderator,
    Administrator,
}

impl FromStr for Role {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "janitor" => Ok(Role::Janitor),
            "moderator" => Ok(Role::Moderator),
            "administrator" => Ok(Role::Administrator),
            _ => Err(Error::UnknownRole {
                role: s.to_string(),
            }),
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
            out.write_all(self.to_string().to_lowercase().as_bytes())?;
            Ok(IsNull::No)
        }
    }

    impl FromSql<Role, Pg> for super::Role {
        fn from_sql(bytes: Option<&[u8]>) -> DeserializeResult<Self> {
            std::str::from_utf8(not_none!(bytes))?
                .parse::<super::Role>()
                .map_err(|err| err.to_string().into())
        }
    }
}

/// An ID for an anonymous site user.
pub type UserId = i32;

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

    /// Hash a user's IP address.
    pub fn hash_ip(ip: IpAddr) -> String {
        let salt = b"longboard-user";
        let conf = argon2::Config::default();

        let octets = match ip {
            IpAddr::V4(v4_addr) => v4_addr.octets().to_vec(),
            IpAddr::V6(v6_addr) => v6_addr.octets().to_vec(),
        };

        hash_encoded(&octets, salt, &conf)
            .expect("could not hash IP address with Argon2")
    }
}

/// An ID for a staff member action.
type StaffActionId = i32;

/// An action done by a staff member.
#[derive(Debug, Queryable, Serialize)]
pub struct StaffAction {
    /// The action's ID in the database.
    pub id: StaffActionId,
    /// The staff member who performed the action.
    pub done_by: String,
    /// A description of the action.
    pub action: String,
    /// The reason the action was done.
    pub reason: String,
    /// When the action was done.
    pub time_stamp: DateTime<Utc>,
}

/// Insertable database type for staff member actions.
#[derive(Insertable)]
#[table_name = "staff_action"]
pub struct NewStaffAction {
    pub done_by: String,
    pub action: String,
    pub reason: String,
}

impl<C: InnerConnection> Connection<C> {
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
            .first(&self.inner)?)
    }

    /// Insert a new staff member.
    pub fn insert_staff(&self, new_staff: &Staff) -> Result<()> {
        use crate::schema::staff::dsl::staff;

        insert_into(staff).values(new_staff).execute(&self.inner)?;

        Ok(())
    }

    /// Delete a staff member.
    pub fn delete_staff<S>(&self, name: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        use crate::schema::session::columns::staff_name;
        use crate::schema::session::dsl::session;

        use crate::schema::staff_action::columns::done_by;
        use crate::schema::staff_action::dsl::staff_action;

        use crate::schema::staff::columns::name as column_name;
        use crate::schema::staff::dsl::staff;

        let name = name.as_ref();

        delete(session.filter(staff_name.eq(name))).execute(&self.inner)?;

        delete(staff_action.filter(done_by.eq(name))).execute(&self.inner)?;

        delete(staff.filter(column_name.eq(name))).execute(&self.inner)?;

        Ok(())
    }

    /// Get a session.
    pub fn session<S>(&self, session_id: S) -> Result<Session>
    where
        S: AsRef<str>,
    {
        use crate::schema::session::columns::id;
        use crate::schema::session::dsl::session as session_table;

        let session: DbSession = session_table
            .filter(id.eq(session_id.as_ref()))
            .limit(1)
            .first(&self.inner)?;

        if session.expires < Utc::now() {
            self.delete_session(session.staff_name)?;
            return Err(Error::ExpiredSession);
        }

        use crate::schema::staff::columns::name;
        use crate::schema::staff::dsl::staff as staff_table;

        let staff: Staff = staff_table
            .filter(name.eq(session.staff_name))
            .limit(1)
            .first(&self.inner)?;

        Ok(Session {
            id: session.id,
            expires: session.expires,
            staff,
        })
    }

    /// Insert a session.
    pub fn insert_session(&self, new_session: Session) -> Result<()> {
        use crate::schema::session::dsl::session;

        let new_session = DbSession {
            id: new_session.id,
            expires: new_session.expires,
            staff_name: new_session.staff.name,
        };

        insert_into(session)
            .values(new_session)
            .execute(&self.inner)?;

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
            .execute(&self.inner)?;

        Ok(())
    }

    /// Get a user by their IP.
    pub fn user(&self, user_ip: IpAddr) -> Result<User> {
        use crate::schema::anon_user::columns::ip;
        use crate::schema::anon_user::dsl::anon_user;

        Ok(anon_user
            .filter(ip.eq(user_ip.to_string()))
            .limit(1)
            .first(&self.inner)?)
    }

    /// Get all users.
    pub fn all_users(&self) -> Result<Vec<User>> {
        use crate::schema::anon_user::dsl::anon_user;

        Ok(anon_user.load(&self.inner)?)
    }

    /// Get the total number of posts a user has made.
    pub fn user_post_count(&self, user_id: UserId) -> Result<u32> {
        use crate::schema::post::columns::user_id as column_user_id;
        use crate::schema::post::dsl::post;

        let count: i64 = post
            .filter(column_user_id.eq(user_id))
            .count()
            .first(&self.inner)?;

        Ok(count.try_into().unwrap())
    }

    /// Insert a user.
    pub fn insert_user(&self, new_user: &NewUser) -> Result<User> {
        use crate::schema::anon_user::dsl::anon_user;

        let user = insert_into(anon_user)
            .values(new_user)
            .get_result(&self.inner)?;

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
            .execute(&self.inner)?;

        Ok(())
    }

    /// Unban a user.
    pub fn unban_user(&self, user_id: UserId) -> Result<()> {
        use crate::schema::anon_user::columns::{ban_expires, id};
        use crate::schema::anon_user::dsl::anon_user;

        update(anon_user.filter(id.eq(user_id)))
            .set(ban_expires.eq::<Option<DateTime<Utc>>>(None))
            .execute(&self.inner)?;

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
            .execute(&self.inner)?;

        Ok(())
    }

    /// Remove the moderation notes for a user.
    pub fn remove_user_note(&self, user_id: UserId) -> Result<()> {
        use crate::schema::anon_user::columns::{id, note};
        use crate::schema::anon_user::dsl::anon_user;

        update(anon_user.filter(id.eq(user_id)))
            .set(note.eq::<Option<String>>(None))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Delete all of the posts a user has made. Returns the amount of rows
    /// deleted.
    pub fn delete_posts_for_user(&self, id: UserId) -> Result<u32> {
        use crate::schema::post::columns::user_id;
        use crate::schema::post::dsl::post;

        let count: usize =
            delete(post.filter(user_id.eq(id))).execute(&self.inner)?;

        Ok(count.try_into().unwrap())
    }

    /// Get all staff actions.
    pub fn all_staff_actions(&self) -> Result<Vec<StaffAction>> {
        use crate::schema::staff_action::dsl::staff_action;

        Ok(staff_action.load(&self.inner)?)
    }

    /// Get a staff action.
    pub fn staff_action(
        &self,
        action_id: StaffActionId,
    ) -> Result<StaffAction> {
        use crate::schema::staff_action::columns::id;
        use crate::schema::staff_action::dsl::staff_action;

        Ok(staff_action
            .filter(id.eq(action_id))
            .limit(1)
            .first(&self.inner)?)
    }

    /// Record an action that a staff member did.
    pub fn insert_staff_action(
        &self,
        new_action: NewStaffAction,
    ) -> Result<()> {
        use crate::schema::staff_action::dsl::staff_action;

        insert_into(staff_action)
            .values(new_action)
            .execute(&self.inner)?;

        Ok(())
    }
}
