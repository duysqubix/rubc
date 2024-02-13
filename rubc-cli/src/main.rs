use rubc::logger;

fn main() -> rubc::Result<()> {
    logger::setup_logger()?;
    println!("Hello, world!");
    Ok(())
}
