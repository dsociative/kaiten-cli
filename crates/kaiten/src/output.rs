use crate::error::CliError;

pub fn print_json<T: serde::Serialize>(value: &T) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Human-readable label for a user: username, else full name, else numeric id.
pub(crate) fn user_label(user: &kaiten_client::User) -> String {
    user.username
        .clone()
        .or_else(|| user.full_name.clone())
        .unwrap_or_else(|| user.id.to_string())
}

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
