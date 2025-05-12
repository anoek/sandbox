use colored::{ColoredString, Colorize};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::io::{self, Write};
use std::sync::Mutex;

pub struct SandboxLogger {
    level: Mutex<LevelFilter>,
    output: Mutex<Box<dyn Write + Send>>,
    deferred: Mutex<Vec<(Level, ColoredString)>>,
    defer_output: Mutex<bool>,
}

impl SandboxLogger {
    pub fn new(level: LevelFilter) -> &'static Self {
        Box::leak(Box::new(Self {
            level: Mutex::new(level),
            output: Mutex::new(Box::new(io::stderr())),
            deferred: Mutex::new(Vec::new()),
            defer_output: Mutex::new(true),
        }))
    }

    pub fn init(&'static self) -> Result<&'static Self, log::SetLoggerError> {
        log::set_logger(self)?;
        log::set_max_level(LevelFilter::Trace);
        Ok(self)
    }

    pub fn set_level(&self, level: LevelFilter) {
        *self.level.lock().expect("Failed to lock level") = level;
    }

    pub fn print_deferred(&self) {
        {
            let deferred =
                self.deferred.lock().expect("Failed to lock deferred");
            let level_filter =
                *self.level.lock().expect("Failed to lock level");
            let mut output = self.output.lock().expect("Failed to lock output");
            for (level, message) in deferred.iter() {
                if level <= &level_filter {
                    let _ = writeln!(output, "{}", message);
                }
            }
        }
        self.deferred
            .lock()
            .expect("Failed to lock deferred")
            .clear();
        *self
            .defer_output
            .lock()
            .expect("Failed to lock defer_output") = false;
    }
}

impl Log for SandboxLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= *self.level.lock().expect("Failed to lock level")
    }

    fn log(&self, record: &Record) {
        let mut output = self.output.lock().expect("Failed to lock output");
        let (level_str, color) = match record.level() {
            log::Level::Error => {
                (record.level().to_string(), colored::Color::Red)
            }
            log::Level::Warn => {
                (format!("{} ", record.level()), colored::Color::Yellow)
            }
            log::Level::Info => {
                (format!("{} ", record.level()), colored::Color::White)
            }
            log::Level::Debug => {
                (record.level().to_string(), colored::Color::Blue)
            }
            log::Level::Trace => {
                (record.level().to_string(), colored::Color::BrightBlack)
            }
        };
        let level_str = level_str.color(color);
        let line =
            format!("[{}] {}: {}", level_str, record.target(), record.args())
                .color(color);
        if *self
            .defer_output
            .lock()
            .expect("Failed to lock defer_output")
        {
            self.deferred
                .lock()
                .expect("Failed to lock deferred")
                .push((record.level(), line));
        } else if self.enabled(record.metadata()) {
            let _ = writeln!(output, "{}", line);
        }
    }

    fn flush(&self) {
        let _ = self.output.lock().expect("Failed to lock output").flush();
    }
}
