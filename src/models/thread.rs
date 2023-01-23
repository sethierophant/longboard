//! Types related to threads of posts.

use std::convert::TryInto;
use std::fmt::Debug;

use chrono::offset::Utc;
use chrono::DateTime;

use diesel::sql_types::Integer;
use diesel::{delete, insert_into, prelude::*, sql_query, update};

use rocket::uri;

use serde::Serialize;

use crate::models::{Connection, *};
use crate::schema::thread;
use crate::{Error, Result};

/// A thread ID.
pub type ThreadId = i32;

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

/// A new thread to be inserted in the database.
#[derive(Debug, Insertable)]
#[diesel(table_name = thread)]
pub struct NewThread {
    pub subject: String,
    pub board: String,
    pub locked: bool,
    pub pinned: bool,
}

/// Convenience function to convert from diesel's error type into our error
/// type, when we're querying for a thread.
fn conv_thread_error(
    thread_id: ThreadId,
) -> impl FnOnce(diesel::result::Error) -> Error {
    move |e: diesel::result::Error| match e {
        diesel::result::Error::NotFound => Error::ThreadNotFound { thread_id },
        _ => Error::from(e),
    }
}

impl<C, M> Connection<C, M>
where
    C: InnerConnection<M> + diesel::connection::LoadConnection,
    M: diesel::connection::TransactionManager<C>,
{
    /// Get a thread.
    pub fn thread(&mut self, thread_id: ThreadId) -> Result<Thread> {
        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        thread
            .filter(id.eq(thread_id))
            .limit(1)
            .first(&mut self.inner)
            .map_err(conv_thread_error(thread_id))
    }

    /// Insert a new thread into the database.
    pub fn insert_thread(&mut self, new_thread: NewThread) -> Result<ThreadId> {
        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        Ok(insert_into(thread)
            .values(&new_thread)
            .returning(id)
            .get_result(&mut self.inner)?)
    }

    /// Update a thread's bump_date.
    pub fn bump_thread(&mut self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{bump_date, id};
        use crate::schema::thread::dsl::thread;

        use diesel::dsl::now;

        update(thread.filter(id.eq(thread_id)))
            .set(bump_date.eq(now))
            .execute(&mut self.inner)
            .map_err(conv_thread_error(thread_id))?;

        Ok(())
    }

    /// Delete a thread.
    ///
    /// This function will recursively delete all reports, posts, and files
    /// associated with the thread as well.
    pub fn delete_thread(&mut self, tid: ThreadId) -> Result<()> {
        use crate::schema::post::columns::thread as post_thread;
        use crate::schema::post::dsl::post as table_post;
        use crate::schema::thread::columns::id as thread_id;
        use crate::schema::thread::dsl::thread as table_thread;

        self.inner.transaction::<_, Error, _>(|conn| {
            let query = "DELETE FROM report R \
                               USING post P \
                               WHERE R.post = P.id AND P.thread = $1";
            sql_query(query).bind::<Integer, _>(tid).execute(conn)?;

            let query = "DELETE FROM file F \
                               USING post P \
                               WHERE F.post = P.id AND P.thread = $1";
            sql_query(query).bind::<Integer, _>(tid).execute(conn)?;

            delete(table_post.filter(post_thread.eq(tid))).execute(conn)?;

            delete(table_thread.filter(thread_id.eq(tid))).execute(conn)?;

            Ok(())
        })?;

        Ok(())
    }

    /// Get all of the posts in a thread.
    pub fn posts_in_thread(
        &mut self,
        thread_id: ThreadId,
    ) -> Result<Vec<Post>> {
        use crate::schema::post::columns::{id, thread};
        use crate::schema::post::dsl::post;

        post.filter(thread.eq(thread_id))
            .order(id.asc())
            .load(&mut self.inner)
            .map_err(conv_thread_error(thread_id))
    }

    /// Get the number of posts in a thread.
    pub fn thread_post_count(&mut self, thread_id: ThreadId) -> Result<u32> {
        use crate::schema::post::columns::thread;
        use crate::schema::post::dsl::post;

        let count: i64 = post
            .filter(thread.eq(thread_id))
            .count()
            .first(&mut self.inner)
            .map_err(conv_thread_error(thread_id))?;

        Ok(count.try_into().expect("couldn't convert i32 to u32"))
    }

    /// Get the number of files in a thread.
    pub fn thread_file_count(&mut self, thread_id: ThreadId) -> Result<u32> {
        use crate::schema::file::dsl::file;
        use crate::schema::post::dsl::post;
        use crate::schema::thread::columns::id;
        use crate::schema::thread::dsl::thread;

        let count: i64 = thread
            .inner_join(post.inner_join(file))
            .filter(id.eq(thread_id))
            .count()
            .first(&mut self.inner)
            .map_err(conv_thread_error(thread_id))?;

        Ok(count.try_into().unwrap())
    }

    /// Get the first post and up to `limit` recent posts from a thread.
    pub fn preview_thread(
        &mut self,
        thread_id: ThreadId,
        limit: u32,
    ) -> Result<Vec<Post>> {
        use crate::schema::post::columns::{id, thread};
        use crate::schema::post::dsl::post;

        let mut posts: Vec<Post> = Vec::new();

        self.inner.transaction::<_, Error, _>(|conn| {
            let first_post: Post = post
                .filter(thread.eq(thread_id))
                .order(id.asc())
                .limit(1)
                .first(conn)
                .map_err(conv_thread_error(thread_id))?;

            posts = post
                .filter(id.ne(first_post.id))
                .filter(thread.eq(thread_id))
                .order(id.desc())
                .limit(limit.into())
                .load(conn)
                .map_err(conv_thread_error(thread_id))?;

            posts.reverse();
            posts.insert(0, first_post);

            Ok(())
        })?;

        Ok(posts)
    }

    /// Lock a thread.
    pub fn lock_thread(&mut self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, locked};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(locked.eq(true))
            .execute(&mut self.inner)
            .map_err(conv_thread_error(thread_id))?;

        Ok(())
    }

    /// Unlock a thread.
    pub fn unlock_thread(&mut self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, locked};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(locked.eq(false))
            .execute(&mut self.inner)?;

        Ok(())
    }

    /// Pin a thread.
    pub fn pin_thread(&mut self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, pinned};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(pinned.eq(true))
            .execute(&mut self.inner)?;

        Ok(())
    }

    /// Unpin a thread.
    pub fn unpin_thread(&mut self, thread_id: ThreadId) -> Result<()> {
        use crate::schema::thread::columns::{id, pinned};
        use crate::schema::thread::dsl::thread;

        update(thread.filter(id.eq(thread_id)))
            .set(pinned.eq(false))
            .execute(&mut self.inner)?;

        Ok(())
    }

    /// Check whether a thread is locked.
    pub fn is_locked(&mut self, thread_id: ThreadId) -> Result<bool> {
        use crate::schema::thread::columns::{id, locked};
        use crate::schema::thread::dsl::thread;

        Ok(thread
            .filter(id.eq(thread_id))
            .select(locked)
            .limit(1)
            .first(&mut self.inner)?)
    }
}
