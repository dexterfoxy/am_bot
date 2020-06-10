pub trait ExpectExt {
    type Output;

    fn expect_ref(&self, msg: &str) -> &Self::Output;

    fn unwrap_ref(&self) -> &Self::Output {
        self.expect_ref("unwrap failed")
    }
}

impl<T> ExpectExt for Option<T> {
    type Output = T;

    fn expect_ref(&self, msg: &str) -> &Self::Output {
        match self {
            Some(x) => x,
            None => panic!("{}", msg)
        }
    }
}