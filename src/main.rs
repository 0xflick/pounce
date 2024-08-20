use pounce::uci::uci_loop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    uci_loop()?;
    Ok(())
}
