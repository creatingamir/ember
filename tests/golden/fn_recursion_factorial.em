fn fact(n: i64) -> i64 {
    if n <= 1 {
        return 1
    }
    return n * fact(n - 1)
}

fn main() {
    print(fact(0))
    print(fact(1))
    print(fact(5))
    print(fact(20))
}
