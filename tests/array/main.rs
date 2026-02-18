#![allow(unused)]

#[ignore]
fn main() {
    let arr = [1; 3];
    foo(arr, 2, 3, 4);
}

fn foo(arr: [u32; 3], x: u32, y: u32, z: u32) -> u32 {
    let tmp = arr[0] + x;
    let tmp2 = arr[0] + y;

    arr[1] // note this should be in the same AT as arrays hold values of same tag
}
