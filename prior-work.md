# Prior Work

## Foundational AI / RL Research:

### [Programming a Computer for Playing Chess](http://www.tandfonline.com/doi/abs/10.1080/14786445008521796)

- **Authors**: Claude Shannon
- **Published**: 1950
- **Summary**: This paper lays out the basic principles of computer chess, and proposes the use of limited-lookahead search in conjunction with a heuristic evaluation function. This paper can be considered as the foundational document for the field of computer chess, and much of computer game playing in general.

### [Neurogammon: a neural-network backgammon program](http://ieeexplore.ieee.org/document/5726779/)

- **Authors**: Gerald Tesauro
- **Published**: 1990
- **Summary**: Neurogammon is the first machine learning program to win a backgammon tournament, and represents one of the first applications of machine learning & neural networks to game playing. The program was trained by supervised learning on large expert datasets.

### [Temporal Difference Learning and TD-Gammon](https://dl.acm.org/doi/10.1145/203330.203343)

- **Authors**: Gerald Tesauro
- **Published**: 1995
- **Summary**: TD-Gammon was the successor to Neurogammon, and was the first program to achieve human expert performance in backgammon. The program was one of the first to use reinforcement learning, and was trained by self-play, using iterated distillation and amplification in a very similar manner to the later AlphaGo / AlphaZero, and to our system.

### [Efficient Selectivity and Backup Operators in Monte-Carlo Tree Search](https://inria.hal.science/inria-00116992)

- **Authors**: Rémi Coulom
- **Published**: 2006
- **Summary**: This paper describes the UCT algorithm, which is a key component of the AlphaGo / AlphaZero system. UCT applies the Upper Confidence Bound selection strategy to Monte Carlo tree search, and is a key component of the later AlphaGo / AlphaZero systems.

## AlphaGo / AlphaZero:

### [Mastering the game of Go with deep neural networks and tree search](https://doi.org/10.1038/nature16961)

- **Authors**: David Silver, et al.
- **Published**: 2016
- **Summary**: This paper describes the AlphaGo system, which was the first to achieve superhuman performance in the game of Go. The system used a combination of deep neural networks and Monte Carlo tree search, and was trained by supervised learning on human expert games, and by reinforcement learning from self-play.

### [Mastering the game of Go without human knowledge](https://doi.org/10.1038/nature24270)

- **Authors**: David Silver, et al.
- **Published**: 2017
- **Summary**: This paper describes the AlphaGo Zero system, achieves superhuman performance in Go and outperforms the original AlphaGo system, while using no human data whatsoever. The paper additionally makes architectural improvements over the original AlphaGo, and significantly simplifies many aspects of the system.

### [A general reinforcement learning algorithm that masters Chess, Shogi, and Go through self-play](https://doi.org/10.1126/science.aar6404)

- **Authors**: David Silver, et al.
- **Published**: 2018
- **Summary**: This paper describes the AlphaZero system, which achieves superhuman performance in the games of Go, Chess, and Shogi, using a single architecture and training process. The paper is notable for demonstrating the generality of the AlphaZero approach.

## Improvements to AlphaZero:

### [Efficient Learning for AlphaZero via Path Consistency](https://proceedings.mlr.press/v162/zhao22h.html)

- **Authors**: Dengwei Zhao, Shikui Tu, Lei Xu
- **Published**: 2022
- **Summary**: This paper describes a method for improving the learning efficiency of AlphaZero, by using path consistency to reduce the number of training iterations required to achieve a given level of performance.

### [Accelerating Self-Play Learning in Go](https://arxiv.org/abs/1902.10565)

- **Authors**: David Wu
- **Published**: 2019
- **Summary**: This paper describes a number of significant improvements to AlphaZero, many of which are domain-general and can be applied to games other than Go.