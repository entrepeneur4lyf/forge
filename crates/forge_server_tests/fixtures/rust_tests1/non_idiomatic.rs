fn main() {
    let mut sum = 0;
    for i in 1..=10 {
        if i % 2 == 0 {
            sum += i;
        }
    }
    println!("Sum of even numbers: {}", sum);
}
