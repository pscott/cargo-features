#[cfg(feature = "error1")]
fn main() {
	println!("I'm hidden, don't look for me!");
}

#[cfg(feature = "error2")]
fn foo() {
}
