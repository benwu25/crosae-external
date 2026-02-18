#![allow(unused)]

use std::collections::HashMap;

#[ignore]
fn main() {
    let mut v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);

    let x = 1;
    let y = 2;

    foo(&v, x, y);
    // x, y, and v[1] in same set

    let z = 4;
    bar(v, x, z);
    // x, z also in the same set, through v[1]

    // should this be mapping TV -> TV or V -> TV?
    // does it matter?
    let mut hm = HashMap::new();
    hm.insert(1, 10);
    hm.insert(2, 20);
    hm.insert(3, 30);

    // a and b in the same set, through hm[2]
    let a = 5;
    let b = 6;
    baz(&mut hm, 5, 6)
}

fn foo(vec: &Vec<u32>, x: u32, y: u32) -> u32 {
    let a = vec[1] + x;
    let b = vec[1] + y;

    vec[0]
}

fn bar(vec: Vec<u32>, a: u32, b: u32) -> u32 {
    let tmp = vec[1] + b;

    return tmp;
}

fn baz(hm: &mut HashMap<u32, u32>, a: u32, b: u32) {
    let tmp1 = hm.get(&2).unwrap() + a;
    let tmp2 = hm.get(&2).unwrap() + b;
}
