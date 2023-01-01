use question::Answer;
use question::Question;

use crate::Groceries;
use crate::GroceriesItem;
use crate::ReadError;
use crate::ShoppingList;
use std::path::Path;

pub fn run() -> Result<(), ReadError> {
    if crate::Groceries::from_path("groceries.json").is_err() {
        return Err(ReadError::LibraryNotFound);
    } else {
        let mut shopping_list = ShoppingList::new();

        if Path::new("list.json").exists() {
            let res = Question::new("Use most recently saved list?")
                .default(question::Answer::NO)
                .show_defaults()
                .confirm();

            if res == Answer::YES {
                let path = "list.json";
                shopping_list = ShoppingList::from_path(path)?;
            }

            // view list if using saved list
            shopping_list.prompt_view_list()?;
        }
        shopping_list.prompt_add_recipes()?;

        shopping_list.prompt_add_groceries()?;

        shopping_list.prompt_save_list()?;
    }
    Ok(())
}

impl ShoppingList {
    pub(crate) fn prompt_view_list(&self) -> Result<(), ReadError> {
        if !self.groceries.is_empty() {
            let res = Question::new("Print shopping list?")
                .default(question::Answer::NO)
                .show_defaults()
                .confirm();

            if res == Answer::YES {
                self.print();
                println!();
            }
        }
        Ok(())
    }

    pub(crate) fn prompt_add_recipes(&mut self) -> Result<(), ReadError> {
        while Question::new("Add more recipe ingredients to our list?")
            .default(question::Answer::NO)
            .show_defaults()
            .confirm()
            == Answer::YES
        {
            let groceries = crate::Groceries::from_path("groceries.json")?;

            for recipe in groceries.recipes.into_iter() {
                let res = Question::new(&format!(
                    "Shall we add {}? (*y*, *n* for next recipe, *s* to skip to end of recipes)",
                    recipe
                ))
                .acceptable(vec!["y", "n", "s"])
                .until_acceptable()
                .default(Answer::RESPONSE("n".to_string()))
                .ask();

                match res {
                    Some(Answer::RESPONSE(res)) if &res == "y" => {
                        if !self.recipes.contains(&recipe) {
                            self.add_recipe(recipe);
                        }
                    }
                    Some(Answer::RESPONSE(res)) if &res == "s" => break,
                    _ => continue,
                }
            }
        }
        Ok(())
    }

    pub(crate) fn prompt_add_groceries(&mut self) -> Result<(), ReadError> {
        while Question::new("Add groceries to shopping list?")
            .default(question::Answer::NO)
            .show_defaults()
            .confirm()
            == Answer::YES
        {
            self.add_groceries()?;
        }
        Ok(())
    }

    pub(crate) fn add_groceries(&mut self) -> Result<(), ReadError> {
        // move everything off list to temp list
        let list_items: Vec<GroceriesItem> = self.groceries.drain(..).collect();
        assert!(self.groceries.is_empty());
        let sections = vec!["fresh", "pantry", "dairy", "protein", "freezer"];
        let groceries = Groceries::from_path("groceries.json")?;
        let groceries_by_section: Vec<Vec<GroceriesItem>> = {
            sections
                .into_iter()
                .map(|section| {
                    let mut a: Vec<GroceriesItem> = list_items
                        .iter()
                        .filter(|groceriesitem| groceriesitem.section.0 == section)
                        .cloned()
                        .collect();

                    let b: Vec<GroceriesItem> = groceries
                        .collection
                        .iter()
                        .filter(|groceriesitem| {
                            groceriesitem.section.0 == section && !a.contains(groceriesitem)
                        })
                        .cloned()
                        .collect();
                    a.extend(b);
                    a
                })
                .collect()
        };
        for section in groceries_by_section {
            if !section.is_empty() {
                for groceriesitem in &section {
                    if !self.groceries.contains(groceriesitem)
                        && groceriesitem
                            .recipes
                            .iter()
                            .any(|recipe| self.recipes.contains(recipe))
                    {
                        self.add_groceries_item(groceriesitem.clone());
                    }
                }
                for groceriesitem in section {
                    if !self.groceries.contains(&groceriesitem) {
                        let res = Question::new(&format!(
                            "Do we need {}? (*y*, *n* for next item, *s* to skip to next section)",
                            groceriesitem.name.0.to_lowercase()
                        ))
                        .acceptable(vec!["y", "n", "s"])
                        .until_acceptable()
                        .default(Answer::RESPONSE("n".to_string()))
                        .ask();

                        match res {
                            Some(Answer::RESPONSE(res)) if &res == "y" => {
                                if !self.groceries.contains(&groceriesitem) {
                                    self.add_groceries_item(groceriesitem.clone());
                                }
                            }
                            Some(Answer::RESPONSE(res)) if &res == "s" => break,
                            _ => continue,
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn prompt_save_list(&mut self) -> Result<(), ReadError> {
        // don't save list if empty
        if !self.checklist.is_empty() && !self.groceries.is_empty() && !self.recipes.is_empty() {
            let res = Question::new("Save current list?")
                .default(question::Answer::NO)
                .show_defaults()
                .confirm();

            if res == Answer::YES {
                self.save()?;
            }

            self.print();
        }
        Ok(())
    }
}
