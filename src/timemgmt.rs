use std::str::FromStr;

use anyhow::Context;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Clock {
    Fixed {
        millis: u64,
    },
    Dynamic {
        p1_base: u64,
        p1_inc: u64,
        p2_base: u64,
        p2_inc: u64,
    },
}

impl Clock {
    fn time_limit(self, is_p1: bool) -> u64 {
        match self {
            Self::Fixed { millis } => millis,
            Self::Dynamic {
                p1_base,
                p1_inc,
                p2_base,
                p2_inc,
            } => {
                let (our_base, our_increment, _, _) = if is_p1 {
                    (p1_base, p1_inc, p2_base, p2_inc)
                } else {
                    (p2_base, p2_inc, p1_base, p1_inc)
                };
                (our_base / 20 + 3 * our_increment / 4).min(our_base - 50)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    nodes: Option<u64>,
    time: Option<Clock>,
}

impl Limits {
    pub const fn movetime(millis: u64) -> Self {
        Self {
            nodes: None,
            time: Some(Clock::Fixed { millis }),
        }
    }

    pub const fn nodes(nodes: u64) -> Self {
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
                p1_base: our_base,
                p1_inc: our_increment,
                p2_base: their_base,
                p2_inc: their_increment,
            }),
        }
    }

    pub const fn infinite() -> Self {
        Self {
            nodes: None,
            time: None,
        }
    }

    pub fn is_out_of_time(&self, nodes_searched: u64, elapsed: u64, is_p1: bool) -> bool {
        if let Some(nodes) = self.nodes {
            if nodes_searched >= nodes {
                return true;
            }
        }
        if let Some(clock) = self.time {
            let time_limit = clock.time_limit(is_p1);
            if elapsed >= time_limit {
                return true;
            }
        }
        false
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::infinite()
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
    type Err = anyhow::Error;

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
                    let nodes = words.next().with_context(|| "nothing after \"nodes\" token!")?.parse()?;
                    components.push(Self::nodes(nodes));
                }
                "movetime" => {
                    let millis = words.next().with_context(|| "nothing after \"movetime\" token!")?.parse()?;
                    components.push(Self::movetime(millis));
                }
                "p1time" => {
                    let p1time = words.next().with_context(|| "nothing after \"p1time\" token!")?.parse()?;
                    let t = words.next().with_context(|| "did not find \"p2time\" token!")?;
                    if t != "p2time" {
                        anyhow::bail!("expected \"p2time\" token, found {:?}", t);
                    }
                    let p2time = words.next().with_context(|| "nothing after \"p2time\" token!")?.parse()?;
                    let t = words.next().with_context(|| "did not find \"p1inc\" token!")?;
                    if t != "p1inc" {
                        anyhow::bail!("expected \"p2time\" token, found {:?}", t);
                    }
                    let p1inc = words.next().with_context(|| "nothing after \"p1inc\" token!")?.parse()?;
                    let t = words.next().with_context(|| "did not find \"p2inc\" token!")?;
                    if t != "p2inc" {
                        anyhow::bail!("expected \"p2time\" token, found {:?}", t);
                    }
                    let p2inc = words.next().with_context(|| "nothing after \"p2inc\" token!")?.parse()?;
                    components.push(Self::time(p1time, p1inc, p2time, p2inc));
                }
                "infinite" => {
                    components.push(Self::infinite());
                }
                _ => anyhow::bail!("unexpected token: {:?}", word),
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
