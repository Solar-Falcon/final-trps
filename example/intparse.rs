fn main() {
    let mut text = String::new();
    let len = std::io::stdin().read_line(&mut text).unwrap();
    assert_eq!(len, text.len());

    let mut ranges = Vec::new();

    for line in text.lines() {
        for elem in line.split(',').map(str::trim) {
            if let Some((start, end)) = elem.split_once("..") {
                let start = start.trim().parse::<i64>().unwrap();
                let end = end.trim().parse::<i64>().unwrap();

                ranges.push(start..=end);
            } else {
                let num = elem.trim().parse::<i64>().unwrap();

                ranges.push(num..=num);
            }
        }
    }

    assert!(!ranges.is_empty());

    println!("ok");
}
