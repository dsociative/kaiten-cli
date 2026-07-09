use crate::error::CliError;

// Called by command handlers starting with Task 11; unused until then.
#[allow(dead_code)]
pub fn print_json<T: serde::Serialize>(value: &T) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[allow(dead_code)]
pub fn table(headers: &[&str]) -> comfy_table::Table {
    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);
    table.set_header(
        headers
            .iter()
            .map(|h| comfy_table::Cell::new(h).add_attribute(comfy_table::Attribute::Bold))
            .collect::<Vec<_>>(),
    );
    table
}
