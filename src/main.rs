mod opcodes;

fn main() {
    let mut mb = opcodes::Motherboard::new();
    let opmap = opcodes::get_op_code_map();
    println!("{:?}", opmap[&0x00](&mut mb, 0x00));
}
