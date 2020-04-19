table! {
    anon_user (id) {
        id -> Int4,
        hash -> Text,
        ban_expires -> Nullable<Timestamptz>,
        note -> Nullable<Text>,
        ip -> Text,
    }
}

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
        user_id -> Int4,
        no_bump -> Bool,
    }
}

table! {
    report (id) {
        id -> Int4,
        time_stamp -> Timestamptz,
        reason -> Text,
        post -> Int4,
        user_id -> Int4,
    }
}

table! {
    session (id) {
        id -> Text,
        expires -> Timestamptz,
        staff_name -> Text,
    }
}

table! {
    staff (name) {
        name -> Text,
        password_hash -> Text,
        role -> crate::sql_types::Role,
    }
}

table! {
    staff_action (id) {
        id -> Int4,
        done_by -> Text,
        action -> Text,
        reason -> Text,
        time_stamp -> Timestamptz,
    }
}

table! {
    thread (id) {
        id -> Int4,
        time_stamp -> Timestamptz,
        subject -> Text,
        board -> Text,
        pinned -> Bool,
        locked -> Bool,
        bump_date -> Timestamptz,
    }
}

joinable!(file -> post (post));
joinable!(post -> anon_user (user_id));
joinable!(post -> board (board));
joinable!(post -> thread (thread));
joinable!(report -> anon_user (user_id));
joinable!(report -> post (post));
joinable!(session -> staff (staff_name));
joinable!(staff_action -> staff (done_by));
joinable!(thread -> board (board));

allow_tables_to_appear_in_same_query!(
    anon_user,
    board,
    file,
    post,
    report,
    session,
    staff,
    staff_action,
    thread,
);
