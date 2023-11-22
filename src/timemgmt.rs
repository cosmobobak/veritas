use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clock {
    Fixed {
        millis: u64,
    },
    Dynamic {
        our_base: u64,
        our_increment: u64,
        their_base: u64,
        their_increment: u64,
    },
}

impl Clock {
    const fn time_limit(self) -> u64 {
        match self {
            Self::Fixed { millis } => millis,
            Self::Dynamic {
                our_base,
                our_increment,
                ..
            } => our_base / 20 + 3 * our_increment / 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    nodes: Option<u64>,
    time: Option<Clock>,
}

impl Limits {
    const fn movetime(millis: u64) -> Self {
        Self {
            nodes: None,
            time: Some(Clock::Fixed { millis }),
        }
    }

    const fn nodes(nodes: u64) -> Self {
        Self {
            nodes: Some(nodes),
            time: None,
        }
    }

    const fn time(
        our_base: u64,
        our_increment: u64,
        their_base: u64,
        their_increment: u64,
    ) -> Self {
        Self {
            nodes: None,
            time: Some(Clock::Dynamic {
                our_base,
                our_increment,
                their_base,
                their_increment,
            }),
        }
    }

    const fn infinite() -> Self {
        Self {
            nodes: None,
            time: None,
        }
    }

    pub const fn is_out_of_time(&self, nodes_searched: u64, elapsed: u64) -> bool {
        if let Some(nodes) = self.nodes {
            if nodes_searched >= nodes {
                return true;
            }
        }
        if let Some(clock) = self.time {
            let time_limit = clock.time_limit();
            if elapsed >= time_limit {
                return true;
            }
        }
        false
    }
}

impl std::ops::Add for Limits {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            nodes: if rhs.nodes.is_some() {
                rhs.nodes
            } else {
                self.nodes
            },
            time: if rhs.time.is_some() {
                rhs.time
            } else {
                self.time
            },
        }
    }
}

impl FromStr for Limits {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // example valid input:
        // "nodes [nodes]" => Self::nodes(nodes)
        // "movetime [ms]" => Self::movetime(ms)
        // "p1time [ms] p2time [ms] p1inc [ms] p2inc [ms]" => Self::time(p1time, p1inc, p2time, p2inc)
        // "infinite" => Self::infinite()
        // "nodes [nodes] movetime [ms]" => Self { nodes: Some(nodes), time: Some(Self::movetime(ms)) }
        // "nodes [nodes] p1time [ms] p2time [ms] p1inc [ms] p2inc [ms]" => Self { nodes: Some(nodes), time: Some(Self::time(p1time, p1inc, p2time, p2inc)) }

        let mut words = s.split_ascii_whitespace();
        let mut components = Vec::with_capacity(4);
        while let Some(word) = words.next() {
            match word {
                "nodes" => {
                    let nodes = words.next().ok_or(())?.parse().map_err(|_| ())?;
                    components.push(Self::nodes(nodes));
                }
                "movetime" => {
                    let millis = words.next().ok_or(())?.parse().map_err(|_| ())?;
                    components.push(Self::movetime(millis));
                }
                "p1time" => {
                    let p1time = words.next().ok_or(())?.parse().map_err(|_| ())?;
                    let _ = words.next().ok_or(())?; // "p2time"
                    let p2time = words.next().ok_or(())?.parse().map_err(|_| ())?;
                    let _ = words.next().ok_or(())?; // "p1inc"
                    let p1inc = words.next().ok_or(())?.parse().map_err(|_| ())?;
                    let _ = words.next().ok_or(())?; // "p2inc"
                    let p2inc = words.next().ok_or(())?.parse().map_err(|_| ())?;
                    components.push(Self::time(p1time, p1inc, p2time, p2inc));
                }
                "infinite" => {
                    components.push(Self::infinite());
                }
                _ => return Err(()),
            }
        }

        Ok(components
            .into_iter()
            .fold(Self::infinite(), |acc, x| acc + x))
    }
}

#[cfg(test)]
mod tests {
    // time-limits parsing
    use super::*;

    #[test]
    fn go_nodes() {
        assert_eq!(Limits::nodes(100), "nodes 100".parse().unwrap());
    }

    #[test]
    fn go_movetime() {
        assert_eq!(Limits::movetime(100), "movetime 100".parse().unwrap());
    }

    #[test]
    fn go_time() {
        assert_eq!(
            Limits::time(100, 10, 200, 20),
            "p1time 100 p2time 200 p1inc 10 p2inc 20".parse().unwrap()
        );
    }

    #[test]
    fn go_infinite() {
        assert_eq!(Limits::infinite(), "infinite".parse().unwrap());
    }

    #[test]
    fn go_nodes_movetime() {
        assert_eq!(
            Limits::nodes(100) + Limits::movetime(100),
            "nodes 100 movetime 100".parse().unwrap()
        );
    }

    #[test]
    fn go_nodes_time() {
        assert_eq!(
            Limits::nodes(100) + Limits::time(100, 10, 200, 20),
            "nodes 100 p1time 100 p2time 200 p1inc 10 p2inc 20"
                .parse()
                .unwrap()
        );
    }

    #[test]
    fn go_nodes_infinite() {
        assert_eq!(
            Limits::nodes(100) + Limits::infinite(),
            "nodes 100 infinite".parse().unwrap()
        );
    }

    #[test]
    fn go_nodes_movetime_time() {
        assert_eq!(
            Limits::nodes(100) + Limits::movetime(100) + Limits::time(100, 10, 200, 20),
            "nodes 100 movetime 100 p1time 100 p2time 200 p1inc 10 p2inc 20"
                .parse()
                .unwrap()
        );
    }
}