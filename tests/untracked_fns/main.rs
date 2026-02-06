fn main() {
    foo(1, 2, 3, 4, 5);
}

fn foo(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 {
    // a and b are being passed outside
    // the tracking boundary, which means
    // there wont be an interaction observed during the 
    // std::cmp::max comparison
    let m_ab = std::cmp::max(a, b);

    // max is within our tracked universe,
    // so therefore interaction between c and d
    // is observed
    let m_cd = max(c, d);

    // m_ab should not have been observed interacting with 
    // anything yet! so let's see that e stays in it's own set.
    return e + m_ab;
}

fn max(a: u32, b: u32) -> u32 {
    if a < b {
        b
    } else {
        a
    }
}
