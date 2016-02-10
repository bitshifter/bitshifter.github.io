#[derive(Debug)]
struct Foo {
    x: i32, y: i32
}

fn main() {
    let foo = Foo { x: 1, y: 2 };
    // println!("{}", foo); // needs core::fmt::Display trait
    println!("{:?}", foo);
    println!("{:#?}", foo);
}
