struct MyStruct {
    x: u32,
    y: u32,
}

fn main() {
    let s = MyStruct { x: 1, y: 2 };
    func(s, 10, 20, 30);
}

fn func(s: MyStruct, x: u32, y: u32, z: u32) -> u32 {
    let a = s.x + x;
    let b = s.y + y;
    let c = s.y + s.x;
    y
}
