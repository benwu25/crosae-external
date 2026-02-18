#![allow(unused)]

// TODO: change this test to use stuff like .collect::<Vec<_>>()
#[ignore]
fn main() {
    let x: u32 = 10;
    let y: u32 = 20;
    let z: f64 = 100.0;

    let v: Vec<Box<u32>> = vec![Box::new(10)];

    foo(x, y, z, &v);
}

fn foo(x: u32, y: u32, z: f64, v: &Vec<Box<u32>>) -> f64 {
    let tmp = *v[0] + x;
    let tmp2 = *v[0] + y;

    z
}
