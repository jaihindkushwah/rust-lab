mod calculator;
use std::io;

fn calculator_recursive(total: i32) {
    println!("Current Total: {}", total);
    println!("Enter operation (+, -, *, /, done):");

    let mut op = String::new();
    io::stdin().read_line(&mut op).unwrap();

    match op.trim() {
        "+" => {
            let value = read_number();
            let result = calculator::add(total, value);
            calculator_recursive(result);
        }

        "-" => {
            let value = read_number();
            let result = calculator::sub(total, value);
            calculator_recursive(result);
        }

        "*" => {
            let value = read_number();
            let result = calculator::mul(total, value);
            calculator_recursive(result);
        }

        "/" => {
            let value = read_number();

            match calculator::div(total, value) {
                Ok(result) => calculator_recursive(result),
                Err(err) => {
                    println!("{}", err);
                    calculator_recursive(total);
                }
            }
        }

        "done" => {
            println!("Final Result: {}", total);
        }

        _ => {
            println!("Invalid operation");
            calculator_recursive(total);
        }
    }
}

fn read_number() -> i32 {
    println!("Enter number:");

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    input.trim().parse().unwrap()
}

fn exec_calculator() {
    println!("Enter initial value:");

    let initial = read_number();

    calculator_recursive(initial);
}
fn main() {
    println!("Hello, world!");
    exec_calculator();
}
