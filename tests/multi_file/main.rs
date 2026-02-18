#![allow(unused)]

mod dep;

#[ignore]
fn main() {
    let mut v = Vec::new();
    foo(1, 2, 3, &mut v);
    let a = dep::from_dep(4, 5, 6);
    println!("{a}");
}

fn foo(x: u32, y: u32, z: u32, v: &mut Vec<u32>) -> u32 {
    v.push(100);
    let tmp = x + v.get(0).unwrap();
    let tmp2 = y + v.get(0).unwrap();

    tmp
}
