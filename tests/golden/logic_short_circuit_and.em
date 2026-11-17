fn noisy(tag: i64, value: bool) -> bool {
    print(tag)
    return value
}

fn main() {
    print(false && noisy(1, true))
    print(true && noisy(2, true))
    print(noisy(3, false) && noisy(4, true))
}
