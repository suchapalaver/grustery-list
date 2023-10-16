use std::fmt::{self, Display};

use common::{
    commands::{Add, ApiCommand, Delete, Read, Update},
    fetcher::{FetchError, Fetcher},
    item::{Item, Name, Section},
    items::Items,
    list::List,
    recipes::{Ingredients, Recipe},
};
use persistence::store::{Storage, Store, StoreError, StoreType};

use thiserror::Error;
use tracing::{info, instrument};
use url::Url;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("fetch error: {0}")]
    FetchError(#[from] FetchError),

    #[error("store error: {0}")]
    StoreError(#[from] StoreError),
}

#[derive(Clone)]
pub struct Api {
    store: Store,
}

impl Api {
    pub async fn new(store: StoreType) -> Result<Self, ApiError> {
        info!("Initializing API with store type: {:?}", store);
        let store = Store::new(store).await?;
        Ok(Self { store })
    }

    #[instrument(level = "debug", skip(self), ret(Debug))]
    pub async fn execute(&mut self, command: ApiCommand) -> Result<ApiResponse, ApiError> {
        match command {
            ApiCommand::Add(cmd) => self.add(cmd).await,
            ApiCommand::Delete(cmd) => self.delete(cmd).await,
            ApiCommand::FetchRecipe(url) => self.fetch_recipe(url).await,
            ApiCommand::MigrateJsonDbToSqlite => self.migrate_json_store_to_sqlite().await,
            ApiCommand::Read(cmd) => self.read(cmd).await,
            ApiCommand::Update(cmd) => self.update(cmd).await,
        }
    }

    async fn add(&mut self, cmd: Add) -> Result<ApiResponse, ApiError> {
        match cmd {
            Add::ChecklistItem(name) => {
                self.store.add_checklist_item(&name).await?;
                Ok(ApiResponse::AddedItem(name))
            }
            Add::Item { name, .. } => {
                self.store.add_item(&name).await?;
                Ok(ApiResponse::AddedItem(name))
            }
            Add::ListItem(name) => {
                self.store.add_list_item(&name).await?;
                Ok(ApiResponse::AddedListItem(name))
            }
            Add::ListRecipe(name) => {
                self.store.add_list_recipe(&name).await?;
                Ok(ApiResponse::AddedListRecipe(name))
            }
            Add::Recipe {
                recipe,
                ingredients,
            } => {
                self.store.add_recipe(&recipe, &ingredients).await?;
                Ok(ApiResponse::AddedRecipe(recipe))
            }
        }
    }

    async fn read(&mut self, cmd: Read) -> Result<ApiResponse, ApiError> {
        match cmd {
            Read::All => {
                let results = self.store.items().await?;
                Ok(ApiResponse::Items(results))
            }
            Read::Checklist => {
                let items = self.store.checklist().await?;
                Ok(ApiResponse::Checklist(items))
            }
            Read::Item(_name) => todo!(),
            Read::List => {
                let list = self.store.list().await?;
                Ok(ApiResponse::List(list))
            }
            Read::ListRecipes => todo!(),
            Read::Recipe(recipe) => match self.store.recipe_ingredients(&recipe).await {
                Ok(Some(ingredients)) => Ok(ApiResponse::RecipeIngredients(ingredients)),
                Ok(None) => Ok(ApiResponse::NothingReturned(ApiCommand::Read(
                    Read::Recipe(recipe),
                ))),
                Err(e) => Err(e.into()),
            },
            Read::Recipes => Ok(ApiResponse::Recipes(self.store.recipes().await?)),
            Read::Sections => {
                let results = self.store.sections().await?;
                Ok(ApiResponse::Sections(results))
            }
        }
    }

    async fn update(&mut self, cmd: Update) -> Result<ApiResponse, ApiError> {
        match cmd {
            Update::Item(_name) => todo!(),
            Update::RefreshList => {
                self.store.refresh_list().await?;
                Ok(ApiResponse::RefreshList)
            }
            Update::Recipe(_name) => todo!(),
        }
    }

    async fn delete(&mut self, cmd: Delete) -> Result<ApiResponse, ApiError> {
        match cmd {
            Delete::ChecklistItem(name) => {
                self.store.delete_checklist_item(&name).await?;
                Ok(ApiResponse::DeletedChecklistItem(name))
            }
            Delete::ClearChecklist => todo!(),
            Delete::ClearList => todo!(),
            Delete::Item(_name) => todo!(),
            Delete::ListItem(_name) => todo!(),
            Delete::Recipe(recipe) => {
                self.store.delete_recipe(&recipe).await?;
                todo!()
            }
        }
    }

    async fn fetch_recipe(&mut self, url: Url) -> Result<ApiResponse, ApiError> {
        let fetcher = Fetcher::from(url);
        let (recipe, ingredients) = fetcher.fetch_recipe().await?;

        self.store.add_recipe(&recipe, &ingredients).await?;
        Ok(ApiResponse::FetchedRecipe((recipe, ingredients)))
    }

    async fn migrate_json_store_to_sqlite(&mut self) -> Result<ApiResponse, ApiError> {
        self.store.migrate_json_store_to_sqlite().await?;
        Ok(ApiResponse::JsonToSqlite)
    }
}

#[derive(Debug)]
pub enum ApiResponse {
    AddedItem(Name),
    AddedListItem(Name),
    AddedListRecipe(Recipe),
    AddedRecipe(Recipe),
    Checklist(Vec<Item>),
    DeletedChecklistItem(Name),
    FetchedRecipe((Recipe, Ingredients)),
    Items(Items),
    JsonToSqlite,
    List(List),
    NothingReturned(ApiCommand),
    Recipes(Vec<Recipe>),
    RecipeIngredients(Ingredients),
    RefreshList,
    Sections(Vec<Section>),
}

impl Display for ApiResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddedItem(name) => writeln!(f, "\nitem added: {name}"),
            Self::AddedListItem(name) => writeln!(f, "\nitem added to list: {name}"),
            Self::AddedListRecipe(recipe) => {
                writeln!(f, "\nrecipe added:\n{recipe}")?;
                Ok(())
            }
            Self::AddedRecipe(name) => writeln!(f, "\nrecipe added: {name}"),
            Self::Checklist(items) => {
                writeln!(f, "\nchecklist:")?;
                for item in items {
                    writeln!(f, "{item}")?;
                }
                Ok(())
            }
            Self::DeletedChecklistItem(name) => writeln!(f, "\ndeleted from checklist: \n{name}"),
            Self::FetchedRecipe((recipe, ingredients)) => {
                writeln!(f, "\n{recipe}:")?;
                for ingredient in ingredients.iter() {
                    writeln!(f, "{ingredient}")?;
                }
                Ok(())
            }
            Self::Items(items) => {
                writeln!(f)?;
                for item in &items.collection {
                    writeln!(f, "{item}")?;
                }
                Ok(())
            }
            Self::JsonToSqlite => writeln!(f, "\nJSON to SQLite data store migration successful"),
            Self::List(list) => {
                writeln!(f)?;
                for item in &list.items {
                    writeln!(f, "{item}")?;
                }
                Ok(())
            }
            Self::NothingReturned(cmd) => writeln!(f, "\nnothing returned for command: {cmd:?}."),
            Self::Recipes(recipes) => {
                writeln!(f)?;
                for recipe in recipes {
                    writeln!(f, "{recipe}")?;
                }
                Ok(())
            }
            Self::RecipeIngredients(ingredients) => {
                writeln!(f)?;
                for ingredient in ingredients.iter() {
                    writeln!(f, "{ingredient}")?;
                }
                Ok(())
            }

            Self::RefreshList => writeln!(f, "\nList is now empty"),
            Self::Sections(sections) => {
                writeln!(f)?;
                for section in sections {
                    writeln!(f, "{section}")?;
                }
                Ok(())
            }
        }
    }
}
