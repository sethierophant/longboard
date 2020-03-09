table! {
    board (name) {
        name -> Text,
        description -> Text,
    }
}

table! {
    file (save_name) {
        save_name -> Text,
        thumb_name -> Nullable<Text>,
        orig_name -> Nullable<Text>,
        content_type -> Nullable<Text>,
        post -> Int4,
        is_spoiler -> Bool,
    }
}

table! {
    post (id) {
        id -> Int4,
        time_stamp -> Timestamptz,
        body -> Text,
        author_name -> Text,
        author_contact -> Nullable<Text>,
        author_ident -> Nullable<Text>,
        thread -> Int4,
        delete_hash -> Nullable<Text>,
        board -> Text,
    }
}

table! {
    report (id) {
        id -> Int4,
        time_stamp -> Timestamptz,
        reason -> Text,
        post -> Int4,
    }
}

table! {
    thread (id) {
        id -> Int4,
        time_stamp -> Timestamptz,
        subject -> Text,
        board -> Text,
    }
}

joinable!(file -> post (post));
joinable!(post -> board (board));
joinable!(post -> thread (thread));
joinable!(report -> post (post));
joinable!(thread -> board (board));

allow_tables_to_appear_in_same_query!(board, file, post, report, thread,);
