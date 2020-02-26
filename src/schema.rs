table! {
    board (name) {
        name -> Text,
        description -> Text,
    }
}

table! {
    post (id) {
        id -> Int4,
        time_stamp -> Timestamptz,
        body -> Text,
        author_name -> Nullable<Text>,
        author_contact -> Nullable<Text>,
        author_ident -> Nullable<Text>,
        thread -> Int4,
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

joinable!(post -> thread (thread));
joinable!(thread -> board (board));

allow_tables_to_appear_in_same_query!(
    board,
    post,
    thread,
);
