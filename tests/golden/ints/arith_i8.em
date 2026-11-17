fn main() {
    i8_max: i8 = 127
    i8_min: i8 = -128
    i8_one: i8 = 1
    i8_neg_one: i8 = -1
    i8_two: i8 = 2
    i8_neg_two: i8 = -2
    i8_zero: i8 = 0
    i8_three: i8 = 3
    i8_seven: i8 = 7
    i8_half_min: i8 = -64
    print(i8_max + i8_neg_one)
    print(i8_max - i8_one)
    print(i8_min + i8_one)
    print(i8_min - i8_neg_one)
    print(i8_max + i8_min)
    print(i8_max - i8_max)
    print(i8_min - i8_min)
    print((i8_max - i8_one) + i8_one)
    print(i8_half_min * i8_two)
    print(i8_max * i8_one)
    print(i8_min * i8_one)
    print(i8_max * i8_neg_one)
    print(i8_min / i8_one)
    print(i8_min / i8_two)
    print(i8_min / i8_neg_two)
    print(i8_max / i8_two)
    print(i8_max % i8_seven)
    print(i8_min % i8_seven)
    print(i8_max % i8_three)
    print(i8_min % i8_three)
    print(-i8_max)
    print(-i8_neg_one)
    print(i8_min < i8_max)
    print(i8_min < i8_zero)
    print(i8_max > i8_min)
    print(i8_max >= i8_max)
    print(i8_min <= i8_min)
    print(i8_max == i8_max)
    print(i8_max != i8_min)
}
