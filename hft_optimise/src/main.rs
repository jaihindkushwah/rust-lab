use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Book {
    title:String,
    author: String,
    year: u32,
    pages: u32,
    price: f32,
}
impl Book {
    fn new(title: String, author: String, year: u32, pages: u32, price: f32) -> Self {
        Book {
            title,
            author,
            year,
            pages,
            price,
        }
    }
}


fn main() {
    let book = Book::new(
        String::from("The Rust Programming Language"),
        String::from("Steve Klabnik and Carol Nichols"),
        2018,
        500,
        39.99
    );
    let mut books: Vec<Book> = Vec::new();
    books.push(book);
    for book in books {
        println!("Book: {}", book.title);
        println!("Author: {}", book.author);
        println!("Year: {}", book.year);
        println!("Pages: {}", book.pages);
        println!("Price: ${:.2}", book.price);
        println!("-----------------------------");
    }
    let json = std::fs::read_to_string("books.json").expect("Failed to read file");
    let books: Vec<Book> = serde_json::from_str(&json).expect("Failed to parse JSON");
    for book in books {
        println!("Book: {}", book.title);
        println!("Author: {}", book.author);
        println!("Year: {}", book.year);
        println!("Pages: {}", book.pages);
        println!("Price: ${:.2}", book.price);
        println!("-----------------------------");
    }
}
