fn main() {
    for i in 0..3 {
        for j in 0..3 {
            if j == i {
                continue
            }
            print(i * 10 + j)
        }
    }
}
