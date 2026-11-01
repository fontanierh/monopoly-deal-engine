use std::fmt::Write;

/// A random 128-bit credential, hex encoded.
pub fn new_token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS random number generator unavailable");
    bytes.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Comparison whose duration does not depend on where the inputs differ.
pub fn tokens_equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Seat credentials plus the host credential that may start a new game.
pub struct Credentials {
    pub host: String,
    pub seats: Vec<String>,
}

impl Credentials {
    pub fn generate(num_seats: usize) -> Self {
        Self {
            host: new_token(),
            seats: (0..num_seats).map(|_| new_token()).collect(),
        }
    }

    /// Checks every seat so timing does not reveal which seat matched.
    pub fn seat_for(&self, token: &str) -> Option<usize> {
        let mut found = None;
        for (i, t) in self.seats.iter().enumerate() {
            if tokens_equal(t, token) {
                found = Some(i);
            }
        }
        found
    }

    pub fn is_host(&self, token: &str) -> bool {
        tokens_equal(&self.host, token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_distinct_and_long() {
        let c = Credentials::generate(6);
        assert_eq!(c.host.len(), 32);
        let mut all: Vec<_> = c.seats.clone();
        all.push(c.host.clone());
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 7);
    }

    #[test]
    fn seat_lookup() {
        let c = Credentials::generate(3);
        assert_eq!(c.seat_for(&c.seats[2]), Some(2));
        assert_eq!(c.seat_for(&c.host), None);
        assert_eq!(c.seat_for(""), None);
        assert!(c.is_host(&c.host));
        assert!(!c.is_host(&c.seats[0]));
    }
}
