// Copyright 2016 Doug Goldstein
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A simple logger to provide symantics similar to what is expected
//! of most UNIX utilities by logging to stderr and the higher the
//! verbosity the higher the log level
//!
//! ### Examples
//!
//! ```rust
//! #[macro_use]
//! extern crate log;
//! extern crate stderrlog;
//!
//! fn main() {
//!     stderrlog::new().module(module_path!()).init().unwrap();
//!
//!     info!("starting up");
//!
//!     // ...
//! }
//! ```
//!
//! ```rust
//! extern crate docopt;
//! #[macro_use]
//! extern crate log;
//! extern crate rustc_serialize;
//! extern crate stderrlog;
//!
//! use docopt::Docopt;
//!
//! const USAGE: &'static str = "
//! Usage: program [-q] [-v...]
//! ";
//!
//! #[derive(Debug, RustcDecodable)]
//! struct Args {
//!     flag_q: bool,
//!     flag_v: usize,
//! }
//!
//! fn main() {
//!     let args: Args = Docopt::new(USAGE)
//!                             .and_then(|d| d.decode())
//!                             .unwrap_or_else(|e| e.exit());
//!
//!     stderrlog::new()
//!             .module(module_path!())
//!             .quiet(args.flag_q)
//!             .timestamp(stderrlog::Timestamp::Second)
//!             .verbosity(args.flag_v)
//!             .init()
//!             .unwrap();
//!     info!("starting up");
//!
//!     // ...
//! }

extern crate log;
extern crate time;
extern crate thread_local;

use log::{LogLevelFilter, LogMetadata};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::collections::Bound;
use std::io::{self, Write};
use thread_local::CachedThreadLocal;

/// State of the timestampping in the logger.
#[derive(Clone, Copy, Debug)]
pub enum Timestamp {
    /// Disable timestamping of log messages
    Off,
    /// Timestamp with second granularity
    Second,
}

#[derive(Debug)]
pub struct StdErrLog {
    verbosity: LogLevelFilter,
    quiet: bool,
    timestamp: Timestamp,
    modules: BTreeSet<String>,
    writer: CachedThreadLocal<RefCell<io::LineWriter<io::Stderr>>>,
}

impl Clone for StdErrLog {
    fn clone(&self) -> StdErrLog {
        StdErrLog {
            verbosity: self.verbosity,
            quiet: self.quiet,
            timestamp: self.timestamp,
            modules: self.modules.clone(),
            writer: CachedThreadLocal::new(),
        }
    }
}

impl log::Log for StdErrLog {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= self.log_level_filter()
    }

    fn log(&self, record: &log::LogRecord) {

        // if logging isn't enabled for this level do a quick out
        if !self.enabled(record.metadata()) {
            return;
        }

        // module we are logging for
        let curr_mod = record.location().module_path();

        // this logger only logs the requested modules unless the
        // vector of modules is empty
        // modules will have module::file in the module_path
        if self.includes_module(curr_mod) {
            let writer =
                self.writer.get_or(|| Box::new(RefCell::new(io::LineWriter::new(io::stderr()))));
            let mut writer = writer.borrow_mut();
            if let Timestamp::Second = self.timestamp {
                let _ = write!(writer, "{} - ", time::now().rfc3339());
            }
            let _ = writeln!(writer, "{} - {}", record.level(), record.args());
        }
    }
}

impl StdErrLog {
    pub fn new() -> StdErrLog {
        StdErrLog {
            verbosity: LogLevelFilter::Error,
            quiet: false,
            timestamp: Timestamp::Off,
            modules: BTreeSet::new(),
            writer: CachedThreadLocal::new(),
        }
    }

    /// Sets the verbosity level of messages that will be displayed
    pub fn verbosity(&mut self, verbosity: usize) -> &mut StdErrLog {
        let log_lvl = match verbosity {
            0 => LogLevelFilter::Error,
            1 => LogLevelFilter::Warn,
            2 => LogLevelFilter::Info,
            3 => LogLevelFilter::Debug,
            _ => LogLevelFilter::Trace,
        };

        self.verbosity = log_lvl;
        self
    }

    pub fn quiet(&mut self, quiet: bool) -> &mut StdErrLog {
        self.quiet = quiet;
        self
    }

    /// Enables or disables the use of timestamps in log messages
    pub fn timestamp(&mut self, timestamp: Timestamp) -> &mut StdErrLog {
        self.timestamp = timestamp;
        self
    }

    pub fn module(&mut self, module: &str) -> &mut StdErrLog {
        self.modules.insert(module.to_owned());
        self
    }

    pub fn modules<T: Into<String>, I: IntoIterator<Item = T>>(&mut self,
                                                               modules: I)
                                                               -> &mut StdErrLog {
        self.modules.extend(modules.into_iter().map(Into::into));
        self
    }

    fn log_level_filter(&self) -> LogLevelFilter {
        if self.quiet {
            LogLevelFilter::Off
        } else {
            self.verbosity
        }
    }

    fn includes_module(&self, module_path: &str) -> bool {
        // If modules is empty, include all module paths
        if self.modules.is_empty() {
            return true;
        }
        // if a prefix of module_path is in `self.modules`, it must
        // be located at the first location before
        // where module_path would be.
        let mut iter = self.modules.range::<str, _>((Bound::Unbounded,
                                                     Bound::Included(module_path)));
        if let Some(prev) = iter.next_back() {
            module_path.starts_with(prev)
        } else {
            false
        }
    }

    pub fn init(&self) -> Result<(), log::SetLoggerError> {
        log::set_logger(|max_log_level| {
                            max_log_level.set(self.log_level_filter());

                            Box::new(self.clone())
                        })
    }
}

pub fn new() -> StdErrLog {
    StdErrLog::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_default_level() {
        extern crate log;

        super::new().module(module_path!()).init().unwrap();

        assert_eq!(log::LogLevel::Error, log::max_log_level())
    }
}
