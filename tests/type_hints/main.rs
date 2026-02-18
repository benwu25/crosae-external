#![allow(unused)]

#[ignore]
fn main() {
    // these type hints need to be replaced with correct
    // values during mutating pass, otherwise compilation will
    // fail
    let x: u32 = 10;
    let y: u32 = 20;
    let z: f64 = 100.0;

    // should become Vec<Box<TaggedValue<u32>>>
    let v: Vec<Box<u32>> = vec![Box::new(10)];

    foo(x, y, z, &v);
}

fn foo(x: u32, y: u32, z: f64, v: &Vec<Box<u32>>) -> f64 {
    let tmp = *v[0] + x;
    let tmp2 = *v[0] + y;

    z
}
