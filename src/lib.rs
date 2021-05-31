use log::{LevelFilter, SetLoggerError};
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};

pub use cubesat::*;
pub mod cubesat;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name

pub fn supermajority(n: usize) -> usize {
    (n as f64 / 3.0 * 2.0).ceil() as usize
}

pub fn configure_log_to_file(filename_base: &str) -> Result<(), SetLoggerError> {
    let date = chrono::Utc::now();
    let path = format!("log/{}-{}.log", filename_base, date);

    let logfile = FileAppender::builder().build(path).unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))
        .unwrap();

    log4rs::init_config(config)?;
    Ok(())
}

pub fn configure_log() -> Result<(), SetLoggerError> {
    let stdout = ConsoleAppender::builder().build();
    let config = log4rs::config::Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))
        .unwrap();

    log4rs::init_config(config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supermajority_test() {
        assert_eq!(supermajority(10), 7);
        assert_eq!(supermajority(25), 17);
        assert_eq!(supermajority(1), 1);
        assert_eq!(supermajority(3), 2);
    }
}
