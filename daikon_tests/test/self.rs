struct X {
    f: i32,
    g: i32,
}

impl X {
    fn foo(&self) {}
}

fn main() {
    let a = X { f: 22, g: 21 };
    a.foo();
}
