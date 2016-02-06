use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::cmp::Ordering;

fn main() {
    println!("Guess the number!");

    let mut buffer = [0; 1];
    File::open("/dev/urandom").unwrap().read(&mut buffer).unwrap();
    let secret_number = (buffer[0] as u32 % 100) + 1;

    loop {
        println!("Please input your guess.");

        let mut guess = String::new();

        io::stdin().read_line(&mut guess)
            .expect("failed to read line");

        let guess: u32 = match guess.trim().parse() {
            Ok(num) => num,
            Err(_) => continue,
        };

        println!("You guessed: {}", guess);

        match guess.cmp(&secret_number) {
            Ordering::Less    => println!("Too small!"),
            Ordering::Greater => println!("Too big!"),
            Ordering::Equal   => {
                println!("You win!");
                break;
            }
        }
    }
}
