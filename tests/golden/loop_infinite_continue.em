fn main() {
    i := 0
    for {
        i += 1
        if i > 6 {
            break
        }
        if i % 3 != 0 {
            continue
        }
        print(i)
    }
}
