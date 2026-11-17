fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() {
    print(add(1, 2))
    print(add(add(1, 2), add(3, 4)))
}
