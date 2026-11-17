fn first_multiple(n: i64, k: i64) -> i64 {
    i := n
    for {
        if i % k == 0 {
            return i
        }
        i += 1
    }
}

fn main() {
    print(first_multiple(10, 7))
}
