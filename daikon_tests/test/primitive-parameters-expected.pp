fn foo(a: i32, b: i32) {
    let mut __daikon_nonce = 0;
    let mut __unwrap_nonce = NONCE_COUNTER.lock().unwrap();
    __daikon_nonce = *__unwrap_nonce;
    *__unwrap_nonce += 1;
    drop(__unwrap_nonce);
    dtrace_entry("foo:::ENTER", __daikon_nonce);
    dtrace_print_prim::<i32>(a, String::from("a"));
    dtrace_print_prim::<i32>(b, String::from("b"));
    dtrace_newline();
    dtrace_exit("foo:::EXIT1", __daikon_nonce);
    dtrace_print_prim::<i32>(a, String::from("a"));
    dtrace_print_prim::<i32>(b, String::from("b"));
    dtrace_newline();
    return;
}

fn main() {
    let mut __daikon_nonce = 0;
    let mut __unwrap_nonce = NONCE_COUNTER.lock().unwrap();
    __daikon_nonce = *__unwrap_nonce;
    *__unwrap_nonce += 1;
    drop(__unwrap_nonce);
    dtrace_entry("main:::ENTER", __daikon_nonce);
    dtrace_newline();
    foo(21, 37);
    dtrace_exit("main:::EXIT1", __daikon_nonce);
    dtrace_newline();
    return;
}
