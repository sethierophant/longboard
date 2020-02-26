use diesel::prelude::*;
use diesel::pg::PgConnection;

use longboard::models::*;

type Error = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Error>;

fn main() -> Result<()> {
    use longboard::schema::board::dsl::board;
    use longboard::schema::thread::dsl::thread;
    use longboard::schema::post::dsl::post;

    let conn = PgConnection::establish(DATABASE_URL)?;

    let boards = board.load::<Board>(&conn)?;
    let threads = thread.load::<Thread>(&conn)?;
    let posts = post.load::<Post>(&conn)?;

    println!("{} boards, {} threads, {} posts",
             boards.len(), threads.len(), posts.len());

    println!("{:#?}", boards);
    println!("{:#?}", threads);
    println!("{:#?}", posts);

    Ok(())
}
