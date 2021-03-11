pub use cubesat::*;
pub mod cubesat;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name

pub fn supermajority(n: usize) -> usize {
    (n as f64 / 3.0 * 2.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supermajority_test() {
        assert_eq!(supermajority(10), 7);
        assert_eq!(supermajority(25), 17);
        assert_eq!(supermajority(1), 1);
    }
}
