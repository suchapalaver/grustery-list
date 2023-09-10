use std::{collections::HashSet, path::Path};

use common::{
    errors::ReadError,
    groceries::Groceries,
    groceriesitem::{Item, Section},
    helpers::ReadWrite,
    recipes::RecipeName,
    shoppinglist::ShoppingList,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};

use crate::{
    models::{self, NewItem, NewItemRecipe, NewItemSection, NewRecipe, NewSection},
    schema,
};

fn load_groceries_library<P: AsRef<Path> + std::marker::Copy>(
    path: P,
) -> Result<Groceries, ReadError> {
    Groceries::from_path(path)
}

fn load_list() -> Result<ShoppingList, ReadError> {
    ShoppingList::from_path("list.json")
}

fn load_groceries_collection<P: AsRef<Path> + std::marker::Copy>(
    path: P,
) -> Result<Vec<Item>, ReadError> {
    Ok(load_groceries_library(path)?.collection)
}

fn load_recipes<P: AsRef<Path> + std::marker::Copy>(path: P) -> Result<Vec<RecipeName>, ReadError> {
    let mut recipes: HashSet<RecipeName> = HashSet::new();

    {
        let groceries = load_groceries_library(path)?;

        for item in groceries.collection {
            if let Some(item_recipes) = item.recipes {
                for recipe in item_recipes {
                    recipes.insert(recipe);
                }
            }
        }

        for recipe in groceries.recipes {
            recipes.insert(recipe);
        }
    }

    {
        let list = load_list()?;

        for recipe in list.recipes {
            recipes.insert(recipe);
        }
    }

    Ok(recipes.into_iter().collect())
}

fn load_sections<P: AsRef<Path> + std::marker::Copy>(path: P) -> Result<Vec<Section>, ReadError> {
    Ok(load_groceries_library(path)?.sections)
}

fn migrate_sections<P: AsRef<Path> + std::marker::Copy>(
    connection: &mut SqliteConnection,
    path: P,
) -> Result<(), ReadError> {
    let sections = load_sections(path)?;

    use crate::schema::sections;

    for name in sections {
        let section = NewSection {
            name: &name.to_string(),
        };

        diesel::insert_into(sections::table)
            .values(&section)
            .on_conflict_do_nothing()
            .execute(connection)
            .expect("Error transferring section");
    }

    Ok(())
}

fn migrate_recipes<P: AsRef<Path> + std::marker::Copy>(
    connection: &mut SqliteConnection,
    path: P,
) -> Result<(), ReadError> {
    let recipes = load_recipes(path)?;

    use crate::schema::recipes;

    for recipe in recipes {
        let recipe = NewRecipe {
            name: &recipe.to_string().to_lowercase(),
        };

        diesel::insert_into(recipes::table)
            .values(&recipe)
            .on_conflict_do_nothing()
            .execute(connection)
            .expect("Error transferring recipe");
    }

    Ok(())
}

pub(crate) fn migrate_groceries<P: AsRef<Path> + std::marker::Copy>(
    connection: &mut SqliteConnection,
    path: P,
) -> Result<(), ReadError> {
    migrate_sections(connection, path)?;
    migrate_recipes(connection, path)?;

    let groceries = load_groceries_collection(path)?;
    let items_table = schema::items::table;
    let recipes_table = schema::recipes::table;
    let sections_table = schema::sections::table;

    for item in groceries {
        // add the item to the item table
        let new_item = NewItem {
            name: &item.name.to_string(),
        };

        diesel::insert_into(items_table)
            .values(&new_item)
            .on_conflict_do_nothing()
            .execute(connection)
            .unwrap_or_else(|_| panic!("Error transferring item {}", item.name));

        // get the item's item_id
        let results = items_table
            .filter(schema::items::dsl::name.eq(item.name.to_string()))
            .load::<models::Item>(connection)
            .expect("Error loading recipes");

        assert_eq!(results.len(), 1);

        let item_id = results[0].id;

        if let Some(item_recipes) = item.recipes {
            // log the item_id in items_recipes
            for r in item_recipes {
                let new_recipe = NewRecipe {
                    name: &r.to_string(),
                };

                diesel::insert_into(schema::recipes::table)
                    .values(&new_recipe)
                    .on_conflict_do_nothing()
                    .execute(connection)
                    .unwrap_or_else(|_| panic!("Error inserting recipe {r}"));

                let results = recipes_table
                    .filter(schema::recipes::dsl::name.eq(r.to_string()))
                    .load::<models::Recipe>(connection)
                    .expect("Error loading recipes");

                assert_eq!(results.len(), 1);

                let recipe_id = results[0].id;

                let new_item_recipe = NewItemRecipe { item_id, recipe_id };

                diesel::insert_into(schema::items_recipes::table)
                    .values(&new_item_recipe)
                    .on_conflict_do_nothing()
                    .execute(connection)
                    .unwrap_or_else(|_| panic!("Error transferring item_recipe for {}", item.name));
            }
        }

        if let Some(item_section) = item.section {
            // log the item_id in items_sections
            let results = sections_table
                .filter(schema::sections::dsl::name.eq(item_section.to_string()))
                .load::<models::Recipe>(connection)
                .expect("Error loading recipes");

            assert_eq!(results.len(), 1);

            for result in results {
                let section_id = result.id;

                let new_item_section = NewItemSection {
                    item_id,
                    section_id,
                };

                diesel::insert_into(schema::items_sections::table)
                    .values(&new_item_section)
                    .on_conflict_do_nothing()
                    .execute(connection)
                    .unwrap_or_else(|_| {
                        panic!("Error transferring item_section for {}", item.name)
                    });
            }
        }
    }

    Ok(())
}
