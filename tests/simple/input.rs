use std::collections::HashMap;

fn main() {
    foo(1, 2, 3);
    let opt = Box::new(5);
}

fn foo(x: u32, y: u32, z: u32) -> u32 {
    let mut v = Vec::new();
    let mut hm = HashMap::new();

    hm.insert(1, 10);
    hm.insert(2, 20);

    v.push(5);

    println!("{:?}", v);

    let tmp  = x + v[0];
    let tmp2 = y + v[0];

    let tmp3 = tmp + *hm.get(&1).unwrap();
    let tmp3 = z + *hm.get(&1).unwrap();

    z
}
