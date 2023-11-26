use gomokugen::board::Board;



pub struct Params {
    pub c_puct: f64,
    pub valuator: Box<dyn Fn(&Board<15>) -> f64>,
}