mod motherboard;
mod opcodes;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut mb = opcodes::Motherboard::new();
    let res = mb.execute_op_code(0x00);
    println!("{:?}", res);
    Ok(())
}
