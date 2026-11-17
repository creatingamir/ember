fn classify(n: i64) -> i64 {
    if n < 0 {
        return -1
    } else if n == 0 {
        return 0
    } else if n < 10 {
        return 1
    } else {
        return 2
    }
}

fn main() {
    print(classify(-5))
    print(classify(0))
    print(classify(7))
    print(classify(100))
}
