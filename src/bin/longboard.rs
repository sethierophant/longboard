use warp::Filter;

use longboard::Result;
use longboard::models::{Database, ThreadId};

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let reply_home = move || {
        format!("longboard {}", env!("CARGO_PKG_VERSION"))
    };
    let home = warp::path::end().map(reply_home);

    let reply_board = move |board_name| {
        let db = Database::open().unwrap();

        format!("{:#?}", db.board(board_name).unwrap())
    };
    let post = warp::path!(String).map(reply_board);

    let reply_thread = move |_board_name, thread_id| {
        let db = Database::open().unwrap();

        format!("{:#?}", db.thread(thread_id).unwrap())
    };
    let thread = warp::path!(String / ThreadId).map(reply_thread);

    let get_routes = home
        .or(post)
        .or(thread);

    let routes = warp::get().and(get_routes);

    warp::serve(routes).run(([0, 0, 0, 0], 3000)).await;

    Ok(())
}
