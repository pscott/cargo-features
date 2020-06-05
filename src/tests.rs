#[cfg(test)]
#[test]
fn my_failing_deny_test() {
    let n = 3;
    for _ in 0..=n {
        println!("pls")
    }

    assert!(2 == 3);
}
