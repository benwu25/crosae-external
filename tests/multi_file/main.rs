mod dep;

fn main() {
    let v = Vec::new();
    foo(1, 2, 3, &v);
    let a = dep::from_dep(4, 5, 6);
    println!("{a}");
}

fn foo(x: u32, y: u32, z: u32, v: &Vec<u32>) -> u32 {
    let tmp = x + y;

    tmp
}
