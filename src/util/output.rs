#![allow(clippy::print_stdout)]

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

static JSON_OUTPUT: LazyLock<Mutex<HashMap<String, Value>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static SHOULD_PRINT_OUTPUT: Mutex<bool> = Mutex::new(true);

pub fn set_should_print_output(should_print: bool) {
    *SHOULD_PRINT_OUTPUT
        .lock()
        .expect("Failed to lock SHOULD_PRINT_OUTPUT") = should_print;
}

pub fn print_output(printed_output: &str) {
    if *SHOULD_PRINT_OUTPUT
        .lock()
        .expect("Failed to lock SHOULD_PRINT_OUTPUT")
    {
        println!("{}", printed_output);
    }
}

pub fn set_json_output(key: &str, value: &Value) {
    JSON_OUTPUT
        .lock()
        .expect("Failed to lock JSON_OUTPUT")
        .insert(key.to_string(), value.clone());
}

#[macro_export]
macro_rules! output {
    ($fmt:expr $(, $args:expr)*) => {
        $crate::util::print_output(&format!($fmt $(, $args)*))
    };
}

#[macro_export]
macro_rules! outln {
    ( $fmt:expr $(, $args:expr)*) => {
        $crate::util::print_output(&format!($fmt $(, $args)*))
    };
}

pub fn print_json_output() -> Result<()> {
    let json_output = JSON_OUTPUT
        .lock()
        .expect("Failed to lock JSON_OUTPUT")
        .clone();
    let map: serde_json::Map<String, Value> = json_output.into_iter().collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&Value::Object(map))
            .context("Error serializing JSON")?
    );
    Ok(())
}
