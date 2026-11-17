fn main() {
    u: u16 = 1000
    u += 500
    print(u)
    u -= 1500
    print(u)
    u += 65535
    print(u)
    u /= 5
    print(u)
    u *= 5
    print(u)
    s: i32 = -2147483648
    s += 2147483647
    print(s)
    s *= -2147483647
    print(s)
    s -= 1
    print(s)
    s /= -2
    print(s)
    s -= 1073741825
    print(s)
}
