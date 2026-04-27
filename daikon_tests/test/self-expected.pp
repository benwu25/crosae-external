struct X {
    f: i32,
    g: i32,
}

impl X {
    fn foo(&self) {
        let mut __daikon_nonce = 0;
        let mut __unwrap_nonce = NONCE_COUNTER.lock().unwrap();
        __daikon_nonce = *__unwrap_nonce;
        *__unwrap_nonce += 1;
        drop(__unwrap_nonce);
        dtrace_entry("foo:::ENTER", __daikon_nonce);
        dtrace_print_pointer(self as *const _ as usize, String::from("self"));
        self.dtrace_print_fields(3, String::from("self"));
        dtrace_newline();
        dtrace_exit("foo:::EXIT1", __daikon_nonce);
        dtrace_print_pointer(self as *const _ as usize, String::from("self"));
        self.dtrace_print_fields(3, String::from("self"));
        dtrace_newline();
        return;
    }
}

fn main() {
    let mut __daikon_nonce = 0;
    let mut __unwrap_nonce = NONCE_COUNTER.lock().unwrap();
    __daikon_nonce = *__unwrap_nonce;
    *__unwrap_nonce += 1;
    drop(__unwrap_nonce);
    dtrace_entry("main:::ENTER", __daikon_nonce);
    dtrace_newline();
    let a = X { f: 22, g: 21 };
    a.foo();
    dtrace_exit("main:::EXIT1", __daikon_nonce);
    dtrace_newline();
    return;
}

impl X<> {
    pub fn dtrace_print_fields(&self, depth: i32, prefix: String) {
        if depth == 0 { return; }
        dtrace_print_prim::<i32>(self.f, format!("{}{}", prefix, ".f"));
        dtrace_print_prim::<i32>(self.g, format!("{}{}", prefix, ".g"));
    }
    pub fn dtrace_print_fields_vec(v: &Vec<&X>, depth: i32, prefix: String) {
        if depth == 0 { return; }
        let mut __daikon_tmp0: Vec<&X> = Vec::new();
        for __daikon_tmp1 in 0..v.len() {
            __daikon_tmp0.push(v[__daikon_tmp1]);
        }
        X::dtrace_print_f_vec(&__daikon_tmp0, format!("{}{}", prefix, ".f"));
        let mut __daikon_tmp2: Vec<&X> = Vec::new();
        for __daikon_tmp3 in 0..v.len() {
            __daikon_tmp2.push(v[__daikon_tmp3]);
        }
        X::dtrace_print_g_vec(&__daikon_tmp2, format!("{}{}", prefix, ".g"));
    }
    pub fn dtrace_print_f_vec(v: &Vec<&X>, var_name: String) {
        let mut traces =
            match File::options().append(true).open("{file_name}.dtrace") {
                Err(why) => panic!("Daikon couldn't open file, {}", why),
                Ok(traces) => traces,
            };
        writeln!(&mut traces, "{}", var_name).ok();
        let mut arr = String::from("[");
        for i in 0..v.len() - 1 { arr.push_str(&format!("{}", v[i].f)); }
        if v.len() > 0 { arr.push_str(&format!("{}", v[v.len() - 1].f)); }
        arr.push_str("]");
        writeln!(&mut traces, "{}", arr).ok();
        writeln!(traces, "0").ok();
    }
    pub fn dtrace_print_g_vec(v: &Vec<&X>, var_name: String) {
        let mut traces =
            match File::options().append(true).open("{file_name}.dtrace") {
                Err(why) => panic!("Daikon couldn't open file, {}", why),
                Ok(traces) => traces,
            };
        writeln!(&mut traces, "{}", var_name).ok();
        let mut arr = String::from("[");
        for i in 0..v.len() - 1 { arr.push_str(&format!("{}", v[i].g)); }
        if v.len() > 0 { arr.push_str(&format!("{}", v[v.len() - 1].g)); }
        arr.push_str("]");
        writeln!(&mut traces, "{}", arr).ok();
        writeln!(traces, "0").ok();
    }
}
