use crate::Groceries;
use crate::GroceriesItem;
use crate::ReadError;

pub fn run() -> Result<(), ReadError> {
    eprintln!(
        "View the groceries in our library?\n\
         --y\n\
         --any other key to continue"
    );

    let path = "groceries.json";
    let mut groceries = Groceries::from_path(path)?;

    while crate::prompt_for_y()? {
        eprintln!();
        groceries.print_groceries();
        eprintln!();
        eprintln!(
            "View the groceries in our library?\n\
                --y\n\
                --any other key to continue"
        );
    }
    eprintln!(
        "Add groceries to our library?\n\
         --y\n\
         --any other key to exit"
    );

    while crate::prompt_for_y()? {
        let new_item = GroceriesItem::new()?;

        if new_item != None {
            groceries.add_item(new_item.unwrap());
        }

        eprintln!(
            "Add more groceries to our library?\n\
         --y\n\
         --any other key to exit"
        );
    }

    groceries.save(path)?;

    Ok(())
}
