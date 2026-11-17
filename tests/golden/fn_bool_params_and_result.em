fn is_positive(n: i64) -> bool {
    return n > 0
}

fn pick(first: bool, a: i64, b: i64) -> i64 {
    if first {
        return a
    }
    return b
}

fn main() {
    print(is_positive(3))
    print(is_positive(-3))
    print(pick(true, 1, 2))
    print(pick(is_positive(0), 1, 2))
}
