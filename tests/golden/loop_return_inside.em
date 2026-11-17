fn first_square_over(limit: i64) -> i64 {
    for i in 1..=limit {
        if i * i > limit {
            return i
        }
    }
    return 0
}

fn main() {
    print(first_square_over(50))
    print(first_square_over(0))
}
