#[derive(Debug)]
struct Foo {
    x: i32, y: i32
}

fn main() {
    let foo = Foo { x: 1, y: 2 };
    // {:?} formatter will use debug trait
    println!("{:?}", foo);
}
