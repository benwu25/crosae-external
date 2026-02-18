#![allow(unused)]

#[ignore]
fn main() {
    implicit_return(1, 2, 3);
    explicit_return(10, 20, 30);
    explicit_unsemi_return(100, 200, 300);

    // Two calls here to merge all params
    nested_explicit_return(1, 2, 99);
    nested_explicit_return(3, 4, 101);

    // same here
    nested_implicit_return(10, 20, 99);
    nested_implicit_return(30, 40, 101);
}

fn implicit_return(x: u32, y: u32, z: u32) -> u32 {
    x + y
}

fn explicit_return(x: u32, y: u32, z: u32) -> u32 {
    return y + z;
}

fn explicit_unsemi_return(x: u32, y: u32, z: u32) -> u32 {
    return x + z;
}

fn nested_implicit_return(x: u32, y: u32, z: u32) -> u32 {
    if z < 100 { x + y } else { x + z }
}

fn nested_explicit_return(x: u32, y: u32, z: u32) -> u32 {
    if z < 100 {
        return x + y;
    } else {
        return x + z;
    }
}
