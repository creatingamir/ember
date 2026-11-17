fn report(n: i64) {
    if n < 0 {
        print(0)
        return
    }
    print(n)
}

fn main() {
    report(-5)
    report(7)
}
