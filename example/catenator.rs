use std::io::BufRead;

fn main() {
    let mut output = String::new();
    let mut stdin = std::io::stdin().lock();

    for _i in 0..6 {
        let mut line = String::new();
        stdin.read_line(&mut line).unwrap();
        output.push_str(line.trim());
    }

    println!("{}", output);
}
