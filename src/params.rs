use gomokugen::board::Board;

use crate::{BOARD_SIZE, game};

pub struct Params {
    pub c_puct: f64,
    pub valuator: Box<dyn Fn(&Board<BOARD_SIZE>) -> f64>,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            c_puct: 10.41,
            valuator: Box::new(|b| game::rollout(*b).into())
        }
    }
}