mod aqua;

#[tokio::main]
async fn main() {
    let result = aqua::add(2, 2);
    println!("Hello, World! {}", result);
    aqua::serve().await.unwrap();
}
