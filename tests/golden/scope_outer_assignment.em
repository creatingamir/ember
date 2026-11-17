fn main() {
    total := 0
    for i in 1..=4 {
        if i % 2 == 0 {
            total += i
        }
    }
    print(total)
}
