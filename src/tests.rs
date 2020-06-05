#[cfg(test)]
#[test]
fn my_failing_deny_test() {
    let n = 3;
    for i in 0..n + 1 {
        println!("pls")
    }

    assert!(2 == 3);
}
