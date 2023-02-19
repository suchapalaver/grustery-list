// @generated automatically by Diesel CLI.

diesel::table! {
    checklist (item_id) {
        item_id -> Integer,
    }
}

diesel::table! {
    items (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    items_recipes (item_id) {
        item_id -> Integer,
        recipe_id -> Integer,
    }
}

diesel::table! {
    items_sections (item_id) {
        item_id -> Integer,
        section_id -> Integer,
    }
}

diesel::table! {
    list (item_id) {
        item_id -> Integer,
    }
}

diesel::table! {
    recipes (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    sections (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::joinable!(checklist -> items (item_id));
diesel::joinable!(items_recipes -> items (item_id));
diesel::joinable!(items_recipes -> recipes (recipe_id));
diesel::joinable!(items_sections -> items (item_id));
diesel::joinable!(items_sections -> sections (section_id));
diesel::joinable!(list -> items (item_id));

diesel::allow_tables_to_appear_in_same_query!(
    checklist,
    items,
    items_recipes,
    items_sections,
    list,
    recipes,
    sections,
);