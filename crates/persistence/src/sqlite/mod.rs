use common::{
    item::Name,
    items::Items,
    list::List,
    recipes::{Ingredients, Recipe},
};
use diesel::{prelude::*, r2d2::ConnectionManager, sqlite::Sqlite, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use r2d2::PooledConnection;

use crate::{
    models::{
        self, Item, ItemInfo, NewChecklistItem, NewItem, NewItemRecipe, NewListItem, NewListRecipe,
        NewRecipe, RecipeModel, Section,
    },
    schema,
    store::{ConnectionPool, Storage, StoreError},
};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub fn run_migrations(connection: &mut impl MigrationHarness<Sqlite>) -> Result<(), StoreError> {
    // This will run the necessary migrations.
    //
    // See the documentation for `MigrationHarness` for
    // all available methods.
    connection.run_pending_migrations(MIGRATIONS)?;

    Ok(())
}

#[derive(Clone)]
pub struct SqliteStore {
    pool: ConnectionPool,
}

impl SqliteStore {
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    pub fn connection(
        &mut self,
    ) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, r2d2::Error> {
        self.pool.get()
    }

    fn get_or_insert_item(
        &mut self,
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<i32, StoreError> {
        diesel::insert_into(schema::items::table)
            .values(NewItem { name })
            .on_conflict_do_nothing()
            .execute(connection)?;

        let item_query = schema::items::table.filter(schema::items::dsl::name.eq(name));

        Ok(item_query
            .select(schema::items::dsl::id)
            .first(connection)?)
    }

    fn get_or_insert_recipe(
        &mut self,
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<i32, StoreError> {
        diesel::insert_into(schema::recipes::table)
            .values(NewRecipe { name })
            .on_conflict_do_nothing()
            .execute(connection)?;

        let recipe_query = schema::recipes::table.filter(schema::recipes::dsl::name.eq(name));

        Ok(recipe_query
            .select(schema::recipes::dsl::id)
            .first(connection)?)
    }

    fn insert_item_recipe(
        &mut self,
        connection: &mut SqliteConnection,
        item_id: i32,
        recipe_id: i32,
    ) -> Result<(), StoreError> {
        diesel::insert_into(schema::items_recipes::table)
            .values(NewItemRecipe { item_id, recipe_id })
            .execute(connection)?;
        Ok(())
    }

    pub fn load_item(
        &mut self,
        connection: &mut SqliteConnection,
        item_id: i32,
    ) -> Result<Vec<Item>, StoreError> {
        Ok(schema::items::table
            .filter(schema::items::dsl::id.eq(&item_id))
            .load::<Item>(connection)?)
    }

    fn get_recipe(
        &mut self,
        connection: &mut SqliteConnection,
        recipe: &str,
    ) -> Result<Option<Vec<RecipeModel>>, StoreError> {
        Ok(schema::recipes::table
            .filter(schema::recipes::dsl::name.eq(recipe))
            .load::<models::RecipeModel>(connection)
            .optional()?)
    }
}

impl Storage for SqliteStore {
    fn add_checklist_item(&mut self, item: &Name) -> Result<(), StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            let id = self.get_or_insert_item(connection, item.as_str())?;
            let query = {
                diesel::insert_into(schema::checklist::table)
                    .values(NewChecklistItem { id })
                    .on_conflict_do_nothing()
            };
            query.execute(connection)?;
            Ok(())
        })
    }

    fn add_item(&mut self, item: &Name) -> Result<(), StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            let item_name = item.to_string();
            let _ = self.get_or_insert_item(connection, &item_name);
            Ok(())
        })
    }

    fn add_list_item(&mut self, item: &Name) -> Result<(), StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            let id = self.get_or_insert_item(connection, item.as_str())?;
            let query = diesel::insert_into(schema::list::table)
                .values(NewListItem { id })
                .on_conflict_do_nothing();
            query.execute(connection)?;
            Ok(())
        })
    }

    fn add_list_recipe(&mut self, recipe: &Recipe) -> Result<(), StoreError> {
        let Some(ingredients) = self.recipe_ingredients(recipe)? else {
            return Err(StoreError::RecipeIngredients(recipe.to_string()));
        };

        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            let id = self.get_or_insert_recipe(connection, recipe.as_str())?;
            diesel::insert_into(schema::list_recipes::table)
                .values(NewListRecipe { id })
                .on_conflict_do_nothing()
                .execute(connection)?;
            for item in ingredients.iter() {
                let item_id = self.get_or_insert_item(connection, item.as_str())?;
                let query = diesel::insert_into(schema::list::table)
                    .values(NewListItem { id: item_id })
                    .on_conflict_do_nothing();
                query.execute(connection)?;

                let new_item_recipe = NewItemRecipe {
                    item_id,
                    recipe_id: id,
                };
                diesel::insert_into(schema::items_recipes::table)
                    .values(&new_item_recipe)
                    .on_conflict_do_nothing()
                    .execute(connection)?;
            }
            Ok(())
        })
    }

    fn add_recipe(&mut self, recipe: &Recipe, ingredients: &Ingredients) -> Result<(), StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            let recipe_id = self.get_or_insert_recipe(connection, recipe.as_str())?;
            let item_ids = ingredients
                .iter()
                .map(|ingredient| self.get_or_insert_item(connection, ingredient.as_str()))
                .collect::<Result<Vec<i32>, _>>()?;

            for item_id in item_ids {
                self.insert_item_recipe(connection, item_id, recipe_id)?;
            }
            Ok(())
        })
    }

    fn checklist(&mut self) -> Result<Vec<common::item::Item>, StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            Ok(schema::items::table
                .filter(
                    schema::items::dsl::id
                        .eq_any(schema::checklist::table.select(schema::checklist::dsl::id)),
                )
                .load::<Item>(connection)?
                .into_iter()
                .map(Into::into)
                .collect())
        })
    }

    fn list(&mut self) -> Result<List, StoreError> {
        let mut list = self.list_items()?;
        list.recipes = self.list_recipes()?;
        list.checklist = self.checklist()?;
        Ok(list)
    }

    fn list_items(&mut self) -> Result<List, StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            Ok(schema::items::table
                .filter(
                    schema::items::dsl::id
                        .eq_any(schema::list::table.select(schema::list::dsl::id)),
                )
                .load::<Item>(connection)?
                .into_iter()
                .map(Into::into)
                .collect::<List>())
        })
    }

    fn list_recipes(&mut self) -> Result<Vec<Recipe>, StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            Ok(schema::recipes::table
                .filter(
                    schema::recipes::dsl::id
                        .eq_any(schema::list_recipes::table.select(schema::list_recipes::dsl::id)),
                )
                .load::<RecipeModel>(connection)?
                .into_iter()
                .map(Into::into)
                .collect())
        })
    }

    fn delete_checklist_item(&mut self, item: &Name) -> Result<(), StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            diesel::delete(
                schema::checklist::table.filter(
                    schema::checklist::dsl::id.eq_any(
                        schema::items::table
                            .select(schema::items::dsl::id)
                            .filter(schema::items::dsl::name.eq(item.as_str())),
                    ),
                ),
            )
            .execute(connection)?;
            Ok(())
        })
    }

    async fn delete_recipe(&mut self, recipe: &Recipe) -> Result<(), StoreError> {
        let mut store = self.clone();
        let recipe = recipe.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = store.connection()?;
            connection.immediate_transaction(|connection| {
                let name = recipe.to_string();
                diesel::delete(
                    schema::items_recipes::table.filter(
                        schema::items_recipes::dsl::recipe_id.eq_any(
                            schema::recipes::table
                                .select(schema::recipes::dsl::id)
                                .filter(schema::recipes::dsl::name.eq(&name)),
                        ),
                    ),
                )
                .execute(connection)?;
                diesel::delete(schema::recipes::table.filter(schema::recipes::dsl::name.eq(name)))
                    .execute(connection)?;
                Ok(())
            })
        })
        .await?
    }

    fn items(&mut self) -> Result<Items, StoreError> {
        use schema::items::dsl::items;
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            Ok(items
                .load::<Item>(connection)?
                .into_iter()
                .map(Into::into)
                .collect())
        })
    }

    fn refresh_list(&mut self) -> Result<(), StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            diesel::delete(schema::list::table).execute(connection)?;
            Ok(())
        })
    }

    fn recipe_ingredients(&mut self, recipe: &Recipe) -> Result<Option<Ingredients>, StoreError> {
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            let Some(results) = self.get_recipe(connection, recipe.as_str())? else {
                return Ok(None);
            };

            let mut v = Vec::<Ingredients>::with_capacity(results.len());

            for recipe in results {
                let recipe_id = recipe.id;

                let results = schema::items_recipes::table
                    .filter(schema::items_recipes::dsl::recipe_id.eq(&recipe_id))
                    .load::<models::ItemRecipe>(connection)?;

                let ingredients = results
                    .iter()
                    .map(|item_recipe| self.load_item(connection, item_recipe.item_id))
                    .collect::<Result<Vec<Vec<Item>>, _>>()?
                    .into_iter()
                    .flatten()
                    .map(|item| Name::from(item.name.as_str()))
                    .collect::<Ingredients>();

                v.push(ingredients);
            }

            Ok(v.into_iter().take(1).next())
        })
    }

    fn sections(&mut self) -> Result<Vec<common::item::Section>, StoreError> {
        use schema::sections::dsl::sections;
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            Ok(sections
                .load::<Section>(connection)?
                .into_iter()
                .map(|sec| sec.name().into())
                .collect())
        })
    }

    fn recipes(&mut self) -> Result<Vec<Recipe>, StoreError> {
        use schema::recipes::dsl::recipes;
        let mut connection = self.connection()?;
        connection.immediate_transaction(|connection| {
            Ok(recipes
                .load::<models::RecipeModel>(connection)?
                .into_iter()
                .map(Into::into)
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::store::{Connection, DatabaseConnector, DbUri};

    use super::*;
    use common::{item::Name, recipes::Ingredients};

    async fn inmem_sqlite_store() -> SqliteStore {
        // Set up a connection to an in-memory SQLite database for testing
        let pool = DatabaseConnector::new(DbUri::from(":memory:".to_string()))
            .try_connect()
            .await
            .unwrap();
        let mut store = SqliteStore::new(pool);
        let mut connection = store.connection().unwrap();
        connection.immediate_transaction(run_migrations).unwrap();
        store
    }

    fn test_item() -> Name {
        Name::from("test item")
    }

    #[tokio::test]
    async fn test_add_checklist_item() {
        let mut store = inmem_sqlite_store().await;

        let item_name = test_item();
        store.add_checklist_item(&item_name).unwrap();

        assert!(store
            .checklist()
            .unwrap()
            .iter()
            .any(|item| item.name() == &item_name));
    }

    #[tokio::test]
    async fn test_add_item() {
        let mut store = inmem_sqlite_store().await;

        let item_name = test_item();
        store.add_item(&item_name).unwrap();

        let items = store.items().unwrap();

        assert!(items
            .collection
            .iter()
            .any(|item| item.name() == &item_name));
    }

    #[tokio::test]
    async fn test_add_list_item() {
        let mut store = inmem_sqlite_store().await;

        let item_name = test_item();
        store.add_list_item(&item_name).unwrap();

        let list = store.list().unwrap();
        let item_in_list = list.items.iter().any(|item| item.name() == &item_name);

        assert!(item_in_list);
    }

    #[tokio::test]
    async fn test_add_list_recipe() {
        let mut store = inmem_sqlite_store().await;

        let ingredients =
            Ingredients::from_iter(vec![Name::from("ingredient 1"), Name::from("ingredient 2")]);

        let recipe = Recipe::new("test recipe").unwrap();
        store.add_recipe(&recipe, &ingredients).unwrap();

        store.add_list_recipe(&recipe).unwrap();

        let list = store.list().unwrap();
        insta::assert_debug_snapshot!(list, @r###"
        List {
            checklist: [],
            recipes: [
                Recipe(
                    "test recipe",
                ),
            ],
            items: [
                Item {
                    name: Name(
                        "ingredient 1",
                    ),
                    section: None,
                    recipes: None,
                },
                Item {
                    name: Name(
                        "ingredient 2",
                    ),
                    section: None,
                    recipes: None,
                },
            ],
        }
        "###);
    }

    #[tokio::test]
    async fn test_add_recipe() {
        let mut store = inmem_sqlite_store().await;

        let ingredients =
            Ingredients::from_iter(vec![Name::from("ingredient 1"), Name::from("ingredient 2")]);

        let recipe = Recipe::new("test recipe").unwrap();
        store.add_recipe(&recipe, &ingredients).unwrap();

        let recipes = store.recipes().unwrap();
        assert_eq!(recipes.len(), 1);

        let added_recipe = &recipes[0];
        assert_eq!(added_recipe.as_str(), "test recipe");

        let recipe_ingredients = store.recipe_ingredients(&recipe).unwrap().unwrap();
        assert_eq!(recipe_ingredients, ingredients);
    }

    #[tokio::test]
    async fn test_delete_checklist_item() {
        let mut store = inmem_sqlite_store().await;

        let item_name = test_item();
        store.add_checklist_item(&item_name).unwrap();

        assert!(store
            .checklist()
            .unwrap()
            .iter()
            .any(|item| item.name() == &item_name));

        store.delete_checklist_item(&item_name).unwrap();

        assert!(store
            .checklist()
            .unwrap()
            .iter()
            .all(|item| item.name() != &item_name));
    }

    #[tokio::test]
    async fn test_delete_recipe() {
        let mut store = inmem_sqlite_store().await;

        let ingredients =
            Ingredients::from_iter(vec![Name::from("ingredient 1"), Name::from("ingredient 2")]);

        let recipe = Recipe::new("test recipe").unwrap();
        store.add_recipe(&recipe, &ingredients).unwrap();

        let recipes = store.recipes().unwrap();
        assert_eq!(recipes.len(), 1);

        let added_recipe = &recipes[0];
        assert_eq!(added_recipe.as_str(), "test recipe");

        let recipe_ingredients = store.recipe_ingredients(&recipe).unwrap().unwrap();
        assert_eq!(recipe_ingredients, ingredients);

        store.delete_recipe(&recipe).unwrap();

        let recipes = store.recipes().unwrap();
        assert_eq!(recipes.len(), 0);

        let recipe_ingredients = store.recipe_ingredients(&recipe).unwrap();
        assert_eq!(recipe_ingredients, None);
    }

    #[tokio::test]
    async fn test_refresh_list() {
        let mut store = inmem_sqlite_store().await;

        store.refresh_list().unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.items.len(), 0);

        let item1 = Name::from("item 1");
        let item2 = Name::from("item 2");
        store.add_list_item(&item1).unwrap();
        store.add_list_item(&item2).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.items.len(), 2);
        assert!(list.items.iter().any(|item| item.name() == &item1));
        assert!(list.items.iter().any(|item| item.name() == &item2));

        store.refresh_list().unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.items.len(), 0);
    }
}
