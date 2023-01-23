//! Types related to boards.

use std::convert::TryInto;
use std::fmt::Debug;

use diesel::dsl::count;
use diesel::sql_types::{Integer, Text};
use diesel::{delete, insert_into, prelude::*, sql_query, update};

use rocket::uri;

use serde::Serialize;

use crate::models::{Connection, *};
use crate::schema::board;
use crate::{Error, Result};

/// A collection of post threads about a similar topic.
#[derive(Debug, Queryable, Serialize, Insertable)]
#[diesel(table_name = board)]
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

/// Convenience function to convert from diesel's error type into our error
/// type, when we're querying for a board.
fn conv_board_error<S>(name: S) -> impl FnOnce(diesel::result::Error) -> Error
where
    S: Into<String>,
{
    move |e: diesel::result::Error| match e {
        diesel::result::Error::NotFound => Error::BoardNotFound {
            board_name: name.into(),
        },
        _ => Error::from(e),
    }
}

impl<C, M> Connection<C, M>
where
    C: InnerConnection<M> + diesel::connection::LoadConnection,
    M: diesel::connection::TransactionManager<C>,
{
    /// Get all boards.
    pub fn all_boards(&mut self) -> Result<Vec<Board>> {
        use crate::schema::board::dsl::board;

        Ok(board.load(&mut self.inner)?)
    }

    /// Get a board.
    pub fn board<S>(&mut self, board_name: S) -> Result<Board>
    where
        S: Into<String>,
    {
        use crate::schema::board::columns::name;
        use crate::schema::board::dsl::board;

        let board_name = board_name.into();

        Ok(board
            .filter(name.eq(&board_name))
            .limit(1)
            .first(&mut self.inner)
            .map_err(conv_board_error(board_name))?)
    }

    /// Insert a new board.
    pub fn insert_board(&mut self, new_board: Board) -> Result<()> {
        use crate::schema::board::dsl::board;

        insert_into(board)
            .values(&new_board)
            .execute(&mut self.inner)?;

        Ok(())
    }

    /// Update a board.
    pub fn update_board<S1, S2>(
        &mut self,
        board_name: S1,
        new_description: S2,
    ) -> Result<()>
    where
        S1: Into<String>,
        S2: AsRef<str>,
    {
        use crate::schema::board::columns::{description, name};
        use crate::schema::board::dsl::board;

        let board_name = board_name.into();

        update(board.filter(name.eq(&board_name)))
            .set(description.eq(new_description.as_ref()))
            .execute(&mut self.inner)
            .map_err(conv_board_error(board_name))?;

        Ok(())
    }

    /// Delete a board.
    ///
    /// This function deletes recursively, it will also delete any threads,
    /// posts, files, and reports associated with the deleted board.
    pub fn delete_board<S>(&mut self, board_name: S) -> Result<()>
    where
        S: Into<String>,
    {
        use crate::schema::board::columns::name;
        use crate::schema::board::dsl::board;

        let board_name = board_name.into();

        self.trim_board(&board_name, 0)?;

        delete(board.filter(name.eq(&board_name)))
            .execute(&mut self.inner)
            .map_err(conv_board_error(board_name))?;

        Ok(())
    }

    /// Trim a board; delete any threads past the thread limit.
    ///
    /// This function deletes recursively, it will also delete any posts, files,
    /// and reports associated with old threads.
    pub fn trim_board<S>(
        &mut self,
        board_name: S,
        max_threads: u32,
    ) -> Result<()>
    where
        S: Into<String>,
    {
        let board_name = board_name.into();

        self.inner.transaction::<_, Error, _>(|conn| {
            let query = "DELETE FROM report R \
                               USING post P, thread T \
                               WHERE R.post = P.id \
                                 AND P.thread = ANY ( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2);";
            sql_query(query)
                .bind::<Text, _>(&board_name)
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(conn)
                .map_err(conv_board_error(&board_name))?;

            let query = "DELETE FROM file F \
                               USING post P, thread T \
                               WHERE F.post = P.id \
                                 AND P.thread = ANY( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2);";
            sql_query(query)
                .bind::<Text, _>(&board_name)
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(conn)
                .map_err(conv_board_error(&board_name))?;

            let query = "DELETE FROM post \
                               WHERE thread = ANY( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2)";
            sql_query(query)
                .bind::<Text, _>(&board_name)
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(conn)
                .map_err(conv_board_error(&board_name))?;

            let query = "DELETE FROM thread \
                               WHERE id = ANY( \
                                     SELECT id FROM thread \
                                      WHERE board = $1 \
                                   ORDER BY bump_date DESC \
                                     OFFSET $2)";
            sql_query(query)
                .bind::<Text, _>(&board_name)
                .bind::<Integer, i32>(max_threads.try_into().unwrap())
                .execute(conn)
                .map_err(conv_board_error(&board_name))?;

            Ok(())
        })?;

        Ok(())
    }

    /// Get a single page of threads on a board.
    ///
    /// The order is the bump order of the thread, i.e. sort by the timestamp of
    /// the most recent post made to the thread which isn't a "no bump" post.
    ///
    /// Pinned threads are always displayed first and the order of pinned
    /// threads is their bump order as well.
    pub fn thread_page<S>(
        &mut self,
        board_name: S,
        page: Page,
    ) -> Result<Vec<Thread>>
    where
        S: Into<String>,
    {
        use crate::schema::thread::columns::{board, bump_date, pinned};
        use crate::schema::thread::dsl::thread;

        let board_name = board_name.into();

        thread
            .filter(board.eq(&board_name))
            .order_by(pinned.desc())
            .then_order_by(bump_date.desc())
            .limit(page.width as i64)
            .offset(page.offset() as i64)
            .load(&mut self.inner)
            .map_err(conv_board_error(board_name))
    }

    /// How many pages of threads there are total.
    pub fn thread_page_count<S>(
        &mut self,
        board_name: S,
        page_width: u32,
    ) -> Result<u32>
    where
        S: Into<String>,
    {
        use crate::schema::thread::columns::{board, id};
        use crate::schema::thread::dsl::thread;

        let board_name = board_name.into();

        let thread_count: i64 = thread
            .filter(board.eq(&board_name))
            .select(count(id))
            .first(&mut self.inner)
            .map_err(conv_board_error(board_name))?;

        Ok((thread_count as f64 / page_width as f64).ceil() as u32)
    }

    /// All of the first posts of threads on the given board.
    ///
    /// The order here is the same as `thread_page`.
    pub fn first_posts<S>(&mut self, board_name: S) -> Result<Vec<Post>>
    where
        S: Into<String>,
    {
        use crate::schema::post::columns as post_columns;
        use crate::schema::thread::columns as thread_columns;
        use crate::schema::thread::dsl::thread;

        let board_name = board_name.into();

        // Here, we create two aliases for the post table, because we're going
        // to use it twice.
        let (inner_post, outer_post) = alias!(
            crate::schema::post as inner_post,
            crate::schema::post as outer_post
        );

        // This SQL statement gets the ID of the first (original) post of a
        // thread, for each thread.
        //
        // It references thread_columns::id, which is possible because of the
        // inner join below.
        let first_post_id = {
            inner_post
                .select(inner_post.field(post_columns::id))
                .filter(
                    inner_post.field(post_columns::id).eq(thread_columns::id),
                )
                .order_by(inner_post.field(post_columns::id).asc())
                .limit(1)
        };

        // Here, we join the two tables, post (aliased to outer_post), and
        // thread. This allows us to use the above SQL statement to filter out
        // only the first posts.
        outer_post
            .inner_join(thread)
            .select(outer_post.fields((
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
            )))
            .filter(outer_post.field(post_columns::board).eq(&board_name))
            .filter(outer_post.field(post_columns::id).eq_any(first_post_id))
            .order_by(thread_columns::pinned.desc())
            .then_order_by(thread_columns::bump_date.desc())
            .load(&mut self.inner)
            .map_err(conv_board_error(board_name))
    }
}
