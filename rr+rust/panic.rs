fn diverges() -> ! {
    panic!("This function never returns!");
}
fn main() {
    diverges()
}
