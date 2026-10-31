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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A writer that keeps everything written to it so tests can inspect it
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Captured {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    fn logger(level: LevelFilter, captured: &Captured) -> SandboxLogger {
        SandboxLogger {
            level: Mutex::new(level),
            output: Mutex::new(Box::new(captured.clone())),
            deferred: Mutex::new(Vec::new()),
            defer_output: Mutex::new(true),
        }
    }

    fn log(logger: &SandboxLogger, level: Level, message: &str) {
        logger.log(
            &Record::builder()
                .level(level)
                .target("test")
                .args(format_args!("{}", message))
                .build(),
        );
    }

    #[test]
    fn test_log_levels() {
        colored::control::set_override(false);
        let captured = Captured::default();
        let logger = logger(LevelFilter::Trace, &captured);
        logger.print_deferred();

        log(&logger, Level::Error, "an error");
        log(&logger, Level::Warn, "a warning");
        log(&logger, Level::Info, "some info");
        log(&logger, Level::Debug, "debugging");
        log(&logger, Level::Trace, "tracing");

        // Warn and info are padded so every level lines up
        assert_eq!(
            captured.text(),
            "[ERROR] test: an error\n\
             [WARN ] test: a warning\n\
             [INFO ] test: some info\n\
             [DEBUG] test: debugging\n\
             [TRACE] test: tracing\n"
        );
    }

    #[test]
    fn test_deferred_output_is_filtered_by_level() {
        colored::control::set_override(false);
        let captured = Captured::default();
        let logger = logger(LevelFilter::Trace, &captured);

        // Messages logged before the level is known are held back...
        log(&logger, Level::Warn, "kept");
        log(&logger, Level::Debug, "dropped");
        assert_eq!(captured.text(), "");

        // ...and only the ones that pass the final level are printed
        logger.set_level(LevelFilter::Warn);
        logger.print_deferred();
        assert_eq!(captured.text(), "[WARN ] test: kept\n");

        // After that, messages are filtered as they're logged
        log(&logger, Level::Debug, "dropped");
        log(&logger, Level::Error, "printed");
        logger.flush();
        assert_eq!(
            captured.text(),
            "[WARN ] test: kept\n[ERROR] test: printed\n"
        );
    }
}
