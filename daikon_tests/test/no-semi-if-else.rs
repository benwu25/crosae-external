/* Description
 * Check that the instrumenter identifies 1 and 2 as
 * exit ppts.
 */

fn test(x: i32) -> i32 {
    if x % 2 == 0 { 1 } else { 2 }
}

fn main() {}
