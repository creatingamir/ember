fn main() {
    for i in 1..=6 {
        if i % 3 == 0 {
            continue
        }
        print(i)
    }
    for i in 1..=100 {
        if i == 3 {
            break
        }
        print(i * 10)
    }
}
