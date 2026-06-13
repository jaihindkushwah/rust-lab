
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
pub fn sub(a: i32, b: i32) -> i32 {
    a - b
}
pub fn mul(a: i32, b: i32) -> i32 {
    a * b
}
pub fn div(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Cannot divide by zero".to_string())
    } else {
        Ok(a / b)
    }
}
// pub fn rem(a: i32, b: i32) -> i32 {
//     a % b
// }
