//! Models and types related to inserting and retrieving data form the database.
//!
//! For some of these model types, there is a different type for data which is
//! inserted into the database and the type which is retrieved from the
//! database.
//!
//! For example, when a new post is created and inserted into the database, the
//! type `NewPost` is used. When a post is retrieved from the database, the type
//! `Post` is used. This pattern is the same for other types.

use std::fmt::Debug;
use std::marker::PhantomData;

use diesel::dsl::exists;
use diesel::r2d2;
use diesel::{prelude::*, select};

use diesel_migrations::{
    embed_migrations, EmbeddedMigrations, MigrationHarness,
};

use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

use crate::{Error, Result};

pub mod board;
pub use board::*;
pub mod thread;
pub use thread::*;
pub mod post;
pub use post::*;
pub mod staff;
pub use staff::*;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

fn run_migrations(
    connection: &mut impl MigrationHarness<diesel::pg::Pg>,
) -> Result<()> {
    connection
        .run_pending_migrations(MIGRATIONS)
        .map_err(Error::from_migration_error)?;

    Ok(())
}

/// A PostgreSQL connection pool.
pub struct ConnectionPool(r2d2::Pool<r2d2::ConnectionManager<PgConnection>>);

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new<S>(uri: S) -> Result<ConnectionPool>
    where
        S: AsRef<str>,
    {
        let manager = r2d2::ConnectionManager::new(uri.as_ref());
        let pool = r2d2::Pool::new(manager)?;

        run_migrations(&mut pool.get()?)?;

        Ok(ConnectionPool(pool))
    }
}

/// A database connection recieved from a pool.
pub type PooledConnection = Connection<
    diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>,
    >,
    diesel::r2d2::PoolTransactionManager<
        diesel::connection::AnsiTransactionManager,
    >,
>;

impl<'a, 'r> FromRequest<'a, 'r> for PooledConnection {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let ConnectionPool(pool) = request
            .guard::<State<ConnectionPool>>()
            .expect("expected database connection pool to be initialized")
            .inner();

        match pool.get() {
            Ok(conn) => Outcome::Success(PooledConnection {
                inner: conn,
                manager: PhantomData,
            }),
            Err(err) => Outcome::Failure((
                Status::InternalServerError,
                Error::from(err),
            )),
        }
    }
}

/// A single, non-pooled connection to the database.
pub type SingleConnection = Connection<
    diesel::pg::PgConnection,
    diesel::connection::AnsiTransactionManager,
>;

impl SingleConnection {
    /// Establish a new non-pooled connection to the database.
    pub fn establish<S>(database_uri: S) -> Result<SingleConnection>
    where
        S: AsRef<str>,
    {
        Ok(SingleConnection {
            inner: PgConnection::establish(database_uri.as_ref())?,
            manager: PhantomData,
        })
    }
}

/// The raw diesel connection.
pub trait InnerConnection<M> = diesel::connection::Connection<
    Backend = diesel::pg::Pg,
    TransactionManager = M,
>;

/// A connection to the database.
///
/// This type is generic, so it could be either either a standalone connection,
/// or a connection obtained from a pool.
///
/// This is the type that most of the database methods are implemented on.
pub struct Connection<C, M>
where
    C: InnerConnection<M> + diesel::connection::LoadConnection,
    M: diesel::connection::TransactionManager<C>,
{
    pub(crate) inner: C,
    manager: std::marker::PhantomData<M>,
}

impl<C, M> Debug for Connection<C, M>
where
    C: InnerConnection<M> + diesel::connection::LoadConnection,
    M: diesel::connection::TransactionManager<C>,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        // It would be nice if we could get more information here but
        // unfortunately this is a pretty opaque type.
        write!(fmt, "<database connection>")
    }
}
