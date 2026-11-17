fn main() {
    for i in 1..=3 {
        for j in 1..=3 {
            if j == 2 {
                break
            }
            print(i * 10 + j)
        }
    }
}
