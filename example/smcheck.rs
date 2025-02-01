const STRING: &str = "     \n";

fn main() {
    let mut string = String::new();
    let len = std::io::stdin().read_line(&mut string).unwrap();
    assert_eq!(len, string.len());

    if &string == STRING {
        println!("ok");
    } else {
        println!("err");
    }
}