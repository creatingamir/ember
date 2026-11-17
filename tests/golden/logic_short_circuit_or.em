fn noisy(tag: i64, value: bool) -> bool {
    print(tag)
    return value
}

fn main() {
    print(true || noisy(1, false))
    print(false || noisy(2, true))
    print(noisy(3, true) || noisy(4, false))
}
