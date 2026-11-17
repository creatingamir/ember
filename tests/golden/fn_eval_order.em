fn tag(n: i64) -> i64 {
    print(n)
    return n
}

fn sum3(a: i64, b: i64, c: i64) -> i64 {
    return a + b + c
}

fn main() {
    print(sum3(tag(1), tag(2), tag(3)))
    print(tag(10) - tag(20))
}
