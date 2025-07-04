use crate::{
    config::Config,
    outln,
    sandbox::{
        Sandbox,
        changes::{
            ChangeEntries, EntryOperation,
            changes::{by_destination, by_reverse_source},
        },
    },
    util::set_json_output,
};
use anyhow::Result;
use log::trace;
use serde_json::{Value, json};

pub fn status(
    config: &Config,
    sandbox: &Sandbox,
    patterns: &[String],
) -> Result<()> {
    trace!("Status of sandbox {}", sandbox.name);

    let cwd = std::env::current_dir()?;
    let all_changes = sandbox.changes(config)?;
    let mut changes = all_changes.matching(&cwd, patterns);
    let non_matching_count =
        ChangeEntries::calculate_non_matching_count(&all_changes, &changes);

    let mut json_output: Vec<Value> = Vec::new();

    // if there are any entries that aren't just place holder entries
    if changes.iter().any(|change| {
        if let EntryOperation::Set(_) = &change.operation {
            change.is_actually_modified()
        } else {
            true
        }
    }) {
        outln!("\nMatching changes:");
        changes.sort_by(by_destination);
        changes
            .iter()
            .filter(|change| {
                matches!(change.operation, EntryOperation::Error(_))
            })
            .try_for_each(|change| -> Result<(), anyhow::Error> {
                outln!("{}", change.display(&cwd)?);
                json_output.push(json!(change.to_json()));
                Ok(())
            })?;

        changes.sort_by(by_reverse_source);
        changes
            .iter()
            .filter(|change| change.operation == EntryOperation::Remove)
            .try_for_each(|change| -> Result<(), anyhow::Error> {
                outln!("{}", change.display(&cwd)?);
                json_output.push(json!(change.to_json()));
                Ok(())
            })?;

        changes.sort_by(by_reverse_source);
        changes
            .iter()
            .filter(|change| change.operation == EntryOperation::Rename)
            .try_for_each(|change| -> Result<(), anyhow::Error> {
                outln!("{}", change.display(&cwd)?);
                json_output.push(json!(change.to_json()));
                Ok(())
            })?;

        changes.sort_by(by_destination);
        changes
            .iter()
            .filter(|change| {
                if let EntryOperation::Set(_) = &change.operation {
                    change.is_actually_modified()
                } else {
                    false
                }
            })
            .try_for_each(|change| -> Result<(), anyhow::Error> {
                outln!("{}", change.display(&cwd)?);
                json_output.push(json!(change.to_json()));
                Ok(())
            })?;
    } else {
        outln!("\nNo matching changes");
    }

    set_json_output("changes", &json!(json_output));

    if non_matching_count > 0 {
        outln!("\n{} external or non-matching changes", non_matching_count);
    }

    outln!("\n");

    Ok(())
}
