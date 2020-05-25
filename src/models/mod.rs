//! Models and types related to the database.

use std::convert::TryInto;
use std::fmt::Debug;
use std::path::PathBuf;

use chrono::offset::Utc;
use chrono::{DateTime, Duration};

use diesel::dsl::{count, exists};
use diesel::r2d2;
use diesel::sql_types::{Integer, Text};
use diesel::{delete, insert_into, prelude::*, select, sql_query, update};

use diesel_migrations::embed_migrations;

use mime::Mime;

use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::{uri, State};

use serde::{Serialize, Serializer};

use crate::schema::{board, file, post, report, thread};
use crate::{Error, Result};

pub mod staff;
pub use staff::*;

embed_migrations!();

/// A thread ID.
pub type ThreadId = i32;
/// A post ID.
pub type PostId = i32;
/// A report ID.
pub type ReportId = i32;

/// A collection of post threads about a similar topic.
#[derive(Debug, Queryable, Serialize, Insertable)]
#[table_name = "board"]
pub struct Board {
    /// The unique name of the board.
    pub name: String,
    /// The description of the board.
    pub description: String,
}

impl Board {
    /// The URI for the board.
    pub fn uri(&self) -> String {
        uri!(crate::routes::board: &self.name, 1).to_string()
    }
}

/// A series of posts about a specific subject.
#[derive(Debug, Queryable, Serialize)]
pub struct Thread {
    /// The ID of the thread.
    pub id: ThreadId,
    /// When the thread was created.
    pub time_stamp: DateTime<Utc>,
    /// The subject of the thread.
    pub subject: String,
    /// The board that this thread was created on.
    pub board_name: String,
    /// Whether or not a thread is pinned to the top of a board.
    pub pinned: bool,
    /// Whether or not a thread is locked from new posts.
    pub locked: bool,
    /// When the thread was last bumped.
    pub bump_date: DateTime<Utc>,
}

impl Thread {
    pub fn uri(&self) -> String {
        uri!(crate::routes::thread: &self.board_name, self.id).to_string()
    }
}

/// A user-made post.
#[derive(Debug, Queryable, Serialize)]
pub struct Post {
    /// The ID of the post.
    pub id: PostId,
    /// When the post was created.
    pub time_stamp: DateTime<Utc>,
    /// The contents of the post.
    pub body: String,
    /// The name of the author.
    pub author_name: String,
    /// A method of contact for the author such as an e-mail address.
    pub author_contact: Option<String>,
    /// The argon2 hash of the identity the user gave.
    pub author_ident: Option<String>,
    /// The thread that this post was posted on.
    pub thread_id: ThreadId,
    /// The argon2 hash of the password the user gave for deletion.
    pub delete_hash: Option<String>,
    /// The board that this post was posted on.
    pub board_name: String,
    /// The user that made the post.
    pub user_id: UserId,
    /// Whether a post should not bump it's thread.
    pub no_bump: bool,
}

impl Post {
    /// The URI of the post.
    pub fn uri(&self) -> String {
        let uri =
            uri!(crate::routes::thread: &self.board_name, &self.thread_id);
        format!("{}#{}", uri, self.id)
    }
}

/// A user-uploaded file.
#[derive(Debug, Serialize)]
pub struct File {
    /// The name the file is saved at.
    pub save_name: String,
    /// The name of the thumbnail of the file, if any.
    pub thumb_name: String,
    /// The original name of the file, if any.
    pub orig_name: Option<String>,
    /// The content-type of the file.
    #[serde(serialize_with = "se_content_type")]
    pub content_type: Mime,
    /// The post that the file belongs to.
    pub post_id: PostId,
    /// Whether or not the file should be hidden by default.
    pub is_spoiler: bool,
}

/// A helper for serializing MIME types.
fn se_content_type<S>(
    content_type: &Mime,
    se: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    content_type.to_string().serialize(se)
}

impl From<DbFile> for File {
    fn from(file: DbFile) -> File {
        File {
            save_name: file.save_name,
            thumb_name: file.thumb_name,
            orig_name: file.orig_name,
            content_type: file.content_type.parse().unwrap(),
            post_id: file.post_id,
            is_spoiler: file.is_spoiler,
        }
    }
}

/// The database representation of a file.
#[derive(Debug, Queryable, Serialize)]
struct DbFile {
    /// The name the file is saved at.
    pub save_name: String,
    /// The name of the thumbnail of the file, if any.
    pub thumb_name: String,
    /// The original name of the file, if any.
    pub orig_name: Option<String>,
    /// The content-type of the file.
    pub content_type: String,
    /// The post that the file belongs to.
    pub post_id: PostId,
    /// Whether or not the file should be hidden by default.
    pub is_spoiler: bool,
}

impl From<File> for DbFile {
    fn from(file: File) -> DbFile {
        DbFile {
            save_name: file.save_name,
            thumb_name: file.thumb_name,
            orig_name: file.orig_name,
            content_type: file.content_type.to_string(),
            post_id: file.post_id,
            is_spoiler: file.is_spoiler,
        }
    }
}

impl File {
    /// The URI of the file.
    pub fn uri(&self) -> String {
        uri!(crate::routes::upload: PathBuf::from(&self.save_name)).to_string()
    }

    /// The URI of the file's thumbnail.
    pub fn thumb_uri(&self) -> String {
        uri!(crate::routes::upload: PathBuf::from(&self.thumb_name)).to_string()
    }
}

/// A report that a user made about a post.
#[derive(Debug, Queryable, Serialize)]
pub struct Report {
    /// The report ID.
    pub id: ReportId,
    /// When the report was made.
    pub time_stamp: DateTime<Utc>,
    /// The reason the post should be removed.
    pub reason: String,
    /// The post.
    pub post_id: PostId,
    /// The user that made the report.
    pub user_id: UserId,
}

/// A new thread to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "thread"]
pub struct NewThread {
    pub subject: String,
    pub board: String,
    pub locked: bool,
    pub pinned: bool,
}

/// A new post to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "post"]
pub struct NewPost {
    pub body: String,
    pub author_name: String,
    pub author_contact: Option<String>,
    pub author_ident: Option<String>,
    pub delete_hash: Option<String>,
    pub thread: ThreadId,
    pub board: String,
    pub user_id: UserId,
    pub no_bump: bool,
}

/// A new file to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "file"]
pub struct NewFile {
    pub save_name: String,
    pub thumb_name: String,
    pub orig_name: Option<String>,
    pub content_type: String,
    pub is_spoiler: bool,
    pub post: PostId,
}

/// A new report to be inserted in the database.
#[derive(Debug, Insertable)]
#[table_name = "report"]
pub struct NewReport {
    pub reason: String,
    pub post: PostId,
    pub user_id: UserId,
}

/// A page location for a paginated resource, for example a page of threads.
pub struct Page {
    /// The page number.
    pub num: u32,
    /// How many items can fit in a page.
    pub width: u32,
}

impl Page {
    /// The offset in items to the start of the page.
    ///
    /// The offset to page 1 is 0.
    pub fn offset(&self) -> u32 {
        (self.num - 1) * self.width
    }
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

        embedded_migrations::run(&pool.get()?)?;

        Ok(ConnectionPool(pool))
    }
}

/// A connection to the database.
///
/// This type is generic, so it could be either either a single connection, or a
/// pooled connection.
pub struct Connection<C: InnerConnection> {
    pub(crate) inner: C,
}

/// The raw diesel connection.
pub trait InnerConnection = diesel::connection::Connection<
    Backend = diesel::pg::Pg,
    TransactionManager = diesel::connection::AnsiTransactionManager,
>;

/// A database connection recieved from a pool.
pub type PooledConnection = Connection<
    diesel::r2d2::PooledConnection<
        diesel::r2d2::ConnectionManager<diesel::pg::PgConnection>,
    >,
>;

/// A single, non-pooled connection to the database.
pub type SingleConnection = Connection<diesel::pg::PgConnection>;

impl<C: InnerConnection> Debug for Connection<C> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        // It would be nice if we could get more information here but
        // unfortunately this is a pretty opaque type.
        write!(fmt, "<database connection>")
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for PooledConnection {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let ConnectionPool(pool) = request
            .guard::<State<ConnectionPool>>()
            .expect("expected database connection pool to be initialized")
            .inner();

        match pool.get() {
            Ok(conn) => Outcome::Success(PooledConnection { inner: conn }),
            Err(err) => Outcome::Failure((
                Status::InternalServerError,
                Error::from(err),
            )),
        }
    }
}

impl SingleConnection {
    /// Establish a new non-pooled connection to the database.
    pub fn establish<S>(database_uri: S) -> Result<SingleConnection>
    where
        S: AsRef<str>,
    {
        Ok(SingleConnection {
            inner: PgConnection::establish(database_uri.as_ref())?,
        })
    }
}

impl<C: InnerConnection> Connection<C> {
    /// Get all boards.
    pub fn all_boards(&self) -> Result<Vec<Board>> {
        use crate::schema::board::dsl::board;

        Ok(board.load(&self.inner)?)
    }

    /// Get a board.
    pub fn board<S>(&self, board_name: S) -> Result<Board>
    where
        S: AsRef<str>,
    {
        use crate::schema::board::columns::name;
        use crate::schema::board::dsl::board;

        Ok(board
            .filter(name.eq(board_name.as_ref()))
            .first(&self.inner)?)
    }

    /// Insert a new board.
    pub fn insert_board(&self, new_board: Board) -> Result<()> {
        use crate::schema::board::dsl::board;

        insert_into(board).values(&new_board).execute(&self.inner)?;

        Ok(())
    }

    /// Update a board.
    pub fn update_board<S>(
        &self,
        board_name: S,
        new_description: S,
    ) -> Result<()>
    where
        S: AsRef<str>,
    {
        use crate::schema::board::columns::{description, name};
        use crate::schema::board::dsl::board;

        update(board.filter(name.eq(board_name.as_ref())))
            .set(description.eq(new_description.as_ref()))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Delete a board.
    ///
    /// FIXME: This function should delete recursively, like e.g. `trim_board`.
    pub fn delete_board<S>(&self, board_name: S) -> Result<()>
    where
        S: AsRef<str>,
    {
        use crate::schema::board::columns::name;
        use crate::schema::board::dsl::board;

        delete(board.filter(name.eq(board_name.as_ref())))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Trim a board; delete any threads past the thread limit.
    ///
    /// This function deletes recursively, it will also delete any posts, files,
    /// and reports associated with old threads. Returns the IDs of the threads
    /// that were deleted.
    pub fn trim_board<S>(&self, board_name: S, max_threads: u32) -> Result<()>
    where
        S: AsRef<str>,
    {
        self.inner.transaction::<_, Error, _>(|| {
            let query = "DELETE FROM report R \
                               USING post P, thread T \
                               WHERE R.post = P.id \
                                 AND P.thread = ANY ( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2);";
            sql_query(query)
                .bind::<Text, _>(board_name.as_ref())
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(&self.inner)?;

            let query = "DELETE FROM file F \
                               USING post P, thread T \
                               WHERE F.post = P.id \
                                 AND P.thread = ANY( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2);";
            sql_query(query)
                .bind::<Text, _>(board_name.as_ref())
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(&self.inner)?;

            let query = "DELETE FROM post \
                               WHERE thread = ANY( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2)";
            sql_query(query)
                .bind::<Text, _>(board_name.as_ref())
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(&self.inner)?;

            let query = "DELETE FROM thread \
                               WHERE id = ANY( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2)";
            sql_query(query)
                .bind::<Text, _>(board_name.as_ref())
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(&self.inner)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Get a thread.
    pub fn thread(&self, thread_id: ThreadId) -> Result<Thread> {
        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(id.eq(thread_id))
            .limit(1)
            .first(&self.inner)?)
    }

    /// Get a single page of threads on a board.
    ///
    /// The order is the bump order of the thread, i.e. sort by the timestamp of
    /// the most recent post made to the thread which isn't a "no bump" post.
    ///
    /// Pinned threads are always displayed first and the order of pinned
    /// threads is their bump order as well.
    pub fn thread_page<S>(
        &self,
        board_name: S,
        page: Page,
    ) -> Result<Vec<Thread>>
    where
        S: AsRef<str>,
    {
        use crate::schema::thread::columns::{board, bump_date, pinned};
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(board.eq(board_name.as_ref()))
            .order_by(pinned.desc())
            .then_order_by(bump_date.desc())
            .limit(page.width as i64)
            .offset(page.offset() as i64)
            .load(&self.inner)?)
    }

    /// How many pages of threads there are total.
    pub fn thread_page_count<S>(
        &self,
        board_name: S,
        page_width: u32,
    ) -> Result<u32>
    where
        S: AsRef<str>,
    {
        use crate::schema::thread::columns::{board, id};
        use crate::schema::thread::dsl::thread;

        let thread_count: i64 = thread
            .filter(board.eq(board_name.as_ref()))
            .select(count(id))
            .first(&self.inner)?;

        Ok((thread_count as f64 / page_width as f64).ceil() as u32)
    }

    /// All of the first posts of threads on the given board.
    ///
    /// The order here is the same as `thread_page`.
    pub fn first_posts<S>(&self, board_name: S) -> Result<Vec<Post>>
    where
        S: AsRef<str>,
    {
        use crate::schema::post::columns as post_columns;
        use crate::schema::post::dsl::post;

        use crate::schema::thread::columns as thread_columns;
        use crate::schema::thread::dsl::thread;

        use diesel::dsl::sql;

        Ok(post
            .inner_join(thread)
            .select((
                post_columns::id,
                post_columns::time_stamp,
                post_columns::body,
                post_columns::author_name,
                post_columns::author_contact,
                post_columns::author_ident,
                post_columns::thread,
                post_columns::delete_hash,
                post_columns::board,
                post_columns::user_id,
                post_columns::no_bump,
            ))
            .filter(post_columns::board.eq(board_name.as_ref()))
            .filter(sql("post.id IN (\
                                 SELECT id FROM post AS inner_post \
                                  WHERE inner_post.thread = thread.id \
                                  ORDER BY id ASC \
                                  LIMIT 1)"))
            .order_by(thread_columns::pinned.desc())
            .then_order_by(thread_columns::bump_date.desc())
            .load(&self.inner)?)
    }

    /// Insert a new thread into the database.
    pub fn insert_thread(&self, new_thread: NewThread) -> Result<ThreadId> {
        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        Ok(insert_into(thread)
            .values(&new_thread)
            .returning(id)
            .get_result(&self.inner)?)
    }

    /// Update a thread's bump_date.
    pub fn bump_thread(&self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{bump_date, id};
        use crate::schema::thread::dsl::thread;

        use diesel::dsl::now;

        update(thread.filter(id.eq(thread_id)))
            .set(bump_date.eq(now))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Delete a thread.
    ///
    /// This function will recursively delete all reports, posts, and files
    /// associated with the thread as well.
    pub fn delete_thread(&self, tid: ThreadId) -> Result<()> {
        use crate::schema::post::columns::thread as post_thread;
        use crate::schema::post::dsl::post as table_post;
        use crate::schema::thread::columns::id as thread_id;
        use crate::schema::thread::dsl::thread as table_thread;

        self.inner.transaction::<_, Error, _>(|| {
            let query = "DELETE FROM report R \
                               USING post P \
                               WHERE R.post = P.id AND P.thread = $1";
            sql_query(query)
                .bind::<Integer, _>(tid)
                .execute(&self.inner)?;

            let query = "DELETE FROM file F \
                               USING post P \
                               WHERE F.post = P.id AND P.thread = $1";
            sql_query(query)
                .bind::<Integer, _>(tid)
                .execute(&self.inner)?;

            delete(table_post.filter(post_thread.eq(tid)))
                .execute(&self.inner)?;

            delete(table_thread.filter(thread_id.eq(tid)))
                .execute(&self.inner)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Get all of the posts in a thread.
    pub fn posts_in_thread(&self, thread_id: ThreadId) -> Result<Vec<Post>> {
        use crate::schema::post::columns::{id, thread};
        use crate::schema::post::dsl::post;

        Ok(post
            .filter(thread.eq(thread_id))
            .order(id.asc())
            .load(&self.inner)?)
    }

    /// Get the number of posts in a thread.
    pub fn thread_post_count(&self, thread_id: ThreadId) -> Result<u32> {
        use crate::schema::post::columns::thread;
        use crate::schema::post::dsl::post;

        let count: i64 = post
            .filter(thread.eq(thread_id))
            .count()
            .first(&self.inner)?;

        Ok(count.try_into().unwrap())
    }

    /// Get the number of files in a thread.
    pub fn thread_file_count(&self, thread_id: ThreadId) -> Result<u32> {
        use crate::schema::file::dsl::file;
        use crate::schema::post::dsl::post;
        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        let count: i64 = thread
            .inner_join(post.inner_join(file))
            .filter(id.eq(thread_id))
            .count()
            .first(&self.inner)?;

        Ok(count.try_into().unwrap())
    }

    /// Get the first post and up to `limit` recent posts from a thread.
    pub fn preview_thread(
        &self,
        thread_id: ThreadId,
        limit: u32,
    ) -> Result<Vec<Post>> {
        use crate::schema::post::columns::{id, thread};
        use crate::schema::post::dsl::post;

        let mut posts: Vec<Post> = Vec::new();

        self.inner.transaction::<_, Error, _>(|| {
            let first_post: Post = post
                .filter(thread.eq(thread_id))
                .order(id.asc())
                .limit(1)
                .first(&self.inner)?;

            posts = post
                .filter(id.ne(first_post.id))
                .filter(thread.eq(thread_id))
                .order(id.desc())
                .limit(limit.into())
                .load(&self.inner)?;

            posts.reverse();
            posts.insert(0, first_post);

            Ok(())
        })?;

        Ok(posts)
    }

    /// Lock a thread.
    pub fn lock_thread(&self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, locked};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(locked.eq(true))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Unlock a thread.
    pub fn unlock_thread(&self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, locked};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(locked.eq(false))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Pin a thread.
    pub fn pin_thread(&self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, pinned};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(pinned.eq(true))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Unpin a thread.
    pub fn unpin_thread(&self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, pinned};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(pinned.eq(false))
            .execute(&self.inner)?;

        Ok(())
    }

    /// Check whether a thread is locked.
    pub fn is_locked(&self, thread_id: ThreadId) -> Result<bool> {
        use crate::schema::thread::columns::{id, locked};
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(id.eq(thread_id))
            .select(locked)
            .limit(1)
            .first(&self.inner)?)
    }

    /// Get a post.
    pub fn post(&self, post_id: PostId) -> Result<Post> {
        use crate::schema::post::columns::id;
        use crate::schema::post::dsl::post;

        Ok(post.filter(id.eq(post_id)).limit(1).first(&self.inner)?)
    }

    /// Insert a new post into the database.
    pub fn insert_post(&self, new_post: NewPost) -> Result<PostId> {
        use crate::schema::post::columns::id;
        use crate::schema::post::dsl::post;

        if self.is_locked(new_post.thread)? {
            return Err(Error::ThreadLocked);
        }

        Ok(insert_into(post)
            .values(&new_post)
            .returning(id)
            .get_result(&self.inner)?)
    }

    /// Delete a post.
    pub fn delete_post(&self, pid: PostId) -> Result<()> {
        self.inner.transaction::<_, Error, _>(|| {
            use crate::schema::file::columns::post as file_post;
            use crate::schema::file::dsl::file as table_file;
            use crate::schema::post::columns::id as post_id;
            use crate::schema::post::dsl::post as table_post;
            use crate::schema::report::columns::post as report_post;
            use crate::schema::report::dsl::report as table_report;

            delete(table_report.filter(report_post.eq(pid)))
                .execute(&self.inner)?;

            delete(table_file.filter(file_post.eq(pid)))
                .execute(&self.inner)?;

            delete(table_post.filter(post_id.eq(pid))).execute(&self.inner)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Get the URI for a post.
    pub fn post_uri(&self, post_id: PostId) -> Result<String> {
        let thread_uri = self.inner.transaction::<_, Error, _>(|| {
            let thread_id: ThreadId = {
                use crate::schema::post::columns::{id, thread};
                use crate::schema::post::dsl::post;

                post.filter(id.eq(post_id))
                    .select(thread)
                    .limit(1)
                    .first(&self.inner)?
            };

            let board_name: String = {
                use crate::schema::thread::columns::{board, id};
                use crate::schema::thread::dsl::thread;

                thread
                    .filter(id.eq(thread_id))
                    .select(board)
                    .limit(1)
                    .first(&self.inner)?
            };

            Ok(uri!(crate::routes::thread: board_name, thread_id))
        })?;

        Ok(format!("{}#{}", thread_uri.to_string(), post_id))
    }

    /// Get the thread that a post belongs to.
    pub fn parent_thread(&self, post_id: PostId) -> Result<Thread> {
        let parent = self.inner.transaction::<_, Error, _>(|| {
            use crate::schema::thread::columns::id;
            use crate::schema::thread::dsl::thread;

            let thread_id: ThreadId = {
                use crate::schema::post::columns::{id, thread};
                use crate::schema::post::dsl::post;

                post.filter(id.eq(post_id))
                    .select(thread)
                    .limit(1)
                    .first(&self.inner)?
            };

            Ok(thread
                .filter(id.eq(thread_id))
                .limit(1)
                .first(&self.inner)?)
        })?;

        Ok(parent)
    }

    /// Check whether a post is the first post in a thread (the original post).
    pub fn is_first_post(&self, post_id: PostId) -> Result<bool> {
        let first_post_id = self.inner.transaction::<_, Error, _>(|| {
            use crate::schema::post::columns::{id, thread};
            use crate::schema::post::dsl::post;

            let Post { thread_id, .. } =
                { post.filter(id.eq(post_id)).limit(1).first(&self.inner)? };

            let first_post_id: i32 = post
                .filter(thread.eq(thread_id))
                .select(id)
                .order(id.asc())
                .limit(1)
                .first(&self.inner)?;

            Ok(first_post_id)
        })?;

        Ok(post_id == first_post_id)
    }

    /// Get all of the files in a post.
    pub fn files_in_post(&self, post_id: PostId) -> Result<Vec<File>> {
        use crate::schema::file::columns::post;
        use crate::schema::file::dsl::file;

        let files: Vec<DbFile> =
            file.filter(post.eq(post_id)).load(&self.inner)?;

        Ok(files.into_iter().map(File::from).collect())
    }

    /// Insert a new file into the database.
    pub fn insert_file(&self, new_file: NewFile) -> Result<()> {
        use crate::schema::file::dsl::file;

        insert_into(file).values(&new_file).execute(&self.inner)?;

        Ok(())
    }

    /// Delete all the files that belong to a post.
    pub fn delete_files_of_post(&self, post_id: PostId) -> Result<()> {
        use crate::schema::file::columns::post;
        use crate::schema::file::dsl::file;
        delete(file.filter(post.eq(post_id))).execute(&self.inner)?;

        Ok(())
    }

    /// Get the number of threads in the database.
    pub fn num_threads(&self) -> Result<i64> {
        use crate::schema::thread::dsl::thread;

        Ok(thread.count().first(&self.inner)?)
    }

    /// Get the number of posts in the database.
    pub fn num_posts(&self) -> Result<i64> {
        use crate::schema::post::dsl::post;

        Ok(post.count().first(&self.inner)?)
    }

    /// Get a report.
    pub fn report(&self, report_id: ReportId) -> Result<Report> {
        use crate::schema::report::columns::id;
        use crate::schema::report::dsl::report;

        Ok(report
            .filter(id.eq(report_id))
            .limit(1)
            .first(&self.inner)?)
    }

    /// Get all post reports.
    pub fn all_reports(&self) -> Result<Vec<Report>> {
        use crate::schema::report::dsl::report;

        Ok(report.load(&self.inner)?)
    }

    /// Insert a new post report.
    pub fn insert_report(&self, new_report: NewReport) -> Result<()> {
        use crate::schema::report::dsl::report;

        insert_into(report)
            .values(&new_report)
            .execute(&self.inner)?;

        Ok(())
    }

    /// Delete a report.
    pub fn delete_report(&self, report_id: ReportId) -> Result<()> {
        use crate::schema::report::columns::id;
        use crate::schema::report::dsl::report;

        delete(report.filter(id.eq(report_id))).execute(&self.inner)?;

        Ok(())
    }

    /// Get up to `limit` recent posts.
    pub fn recent_posts(&self, limit: u32) -> Result<Vec<Post>> {
        use crate::schema::post::columns::time_stamp;
        use crate::schema::post::dsl::post;

        Ok(post
            .order(time_stamp.desc())
            .limit(limit.into())
            .load(&self.inner)?)
    }

    /// Get up to `limit` recently uploaded files.
    pub fn recent_files(&self, limit: u32) -> Result<Vec<File>> {
        use crate::schema::file::columns::*;
        use crate::schema::file::dsl::file;
        use crate::schema::post::columns::time_stamp;
        use crate::schema::post::dsl::post as post_table;

        let files: Vec<DbFile> = file
            .inner_join(post_table)
            .order(time_stamp.desc())
            .limit(limit.into())
            .select((
                save_name,
                thumb_name,
                orig_name,
                content_type,
                post,
                is_spoiler,
            ))
            .load(&self.inner)?;

        Ok(files.into_iter().map(File::from).collect())
    }

    /// Check if the user has made any posts recently.
    pub fn user_rate_limit_exceeded(
        &self,
        user_id: UserId,
        limit: Duration,
    ) -> Result<bool> {
        use crate::schema::post::columns as post_columns;
        use crate::schema::post::dsl::post;

        let query = post
            .filter(post_columns::user_id.eq(user_id))
            .filter(post_columns::time_stamp.gt(Utc::now() - limit));

        Ok(select(exists(query)).get_result(&self.inner)?)
    }

    /// Check if a post has been made with the given content within recently.
    pub fn content_rate_limit_exceeded<S>(
        &self,
        post_body: S,
        limit: Duration,
    ) -> Result<bool>
    where
        S: AsRef<str>,
    {
        use crate::schema::post::columns as post_columns;
        use crate::schema::post::dsl::post;

        let query = post
            .filter(post_columns::body.eq(post_body.as_ref()))
            .filter(post_columns::time_stamp.gt(Utc::now() - limit));

        Ok(select(exists(query)).get_result(&self.inner)?)
    }
}
