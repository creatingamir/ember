fn main() {
    a: i8 = -3
    b: i8 = 3
    for i in a..b {
        print(i)
    }
    lo: i8 = -128
    hi: i8 = -126
    for j in lo..=hi {
        print(j)
    }
    top: i8 = 127
    for k in 125..=top {
        print(k)
    }
}
