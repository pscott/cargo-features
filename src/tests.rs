#[cfg(test)]
#[test]
fn my_failing_deny_test() {
    let n = 3;
    let mut i = 0;
    for _ in 0..=n {
        println!("pls");
        i += 1;
    }

    assert!(i == 6);
}
