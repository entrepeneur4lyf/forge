use std::collections::HashMap;

fn main() {
    let mut numbers = vec![];
    for i in 0..10 {
        numbers.push(i);
    }
    println!("Numbers: {:?}", numbers);

    let mut map = HashMap::new();
    for i in 0..10 {
        map.insert(i, i * 2);
    }

    let mut sum = 0;
    for (key, value) in map.iter() {
        if key % 2 == 0 {
            sum += value;
        }
    }
    println!("Sum of values with even keys: {}", sum);

    let names = vec!["Alice", "Bob", "Charlie", "Alice", "Bob"];
    let mut counts = HashMap::new();
    for name in names {
        if counts.contains_key(&name) {
            *counts.get_mut(&name).unwrap() += 1;
        } else {
            counts.insert(name, 1);
        }
    }
    println!("Name counts: {:?}", counts);

    let numbers = vec![1, 2, 3, 4, 5];
    let doubled_numbers = numbers
        .iter()
        .map(|n| n * 2)
        .collect::<Vec<_>>()
        .into_iter()
        .filter(|n| n % 2 == 0)
        .collect::<Vec<_>>();
    println!("Doubled even numbers: {:?}", doubled_numbers);

    let long_string = "This is a very very very very long string".to_string();
    let mut char_count = 0;
    for c in long_string.chars() {
        if c.is_alphabetic() {
            char_count += 1;
        }
    }
    println!("Alphabetic character count: {}", char_count);
}
