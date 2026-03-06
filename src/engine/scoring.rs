use serde::{Deserialize, Serialize};

use super::bidding::BidSuit;
use super::contract::Contract;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Score {
    Made {
        contract_points: i32,
        overtrick_points: i32,
        game_bonus: i32,
        slam_bonus: i32,
        insult_bonus: i32,
        total: i32,
    },
    Defeated {
        undertricks: u8,
        penalty: i32,
    },
    PassedOut,
}

impl Score {
    #[allow(dead_code)]
    pub fn total_points(&self) -> i32 {
        match self {
            Score::Made { total, .. } => *total,
            Score::Defeated { penalty, .. } => *penalty,
            Score::PassedOut => 0,
        }
    }
}

pub fn calculate_score(contract: &Contract, tricks_won: u8) -> Score {
    let needed = contract.level + 6;

    if tricks_won >= needed {
        let overtricks = tricks_won - needed;
        score_made(contract, overtricks)
    } else {
        let undertricks = needed - tricks_won;
        score_defeated(contract, undertricks)
    }
}

fn trick_value(suit: BidSuit) -> i32 {
    match suit {
        BidSuit::Clubs | BidSuit::Diamonds => 20,
        BidSuit::Hearts | BidSuit::Spades => 30,
        BidSuit::NoTrump => 30, // per trick (first trick handled separately)
    }
}

fn score_made(contract: &Contract, overtricks: u8) -> Score {
    // Contract points
    let base_per_trick = trick_value(contract.suit);
    let mut contract_points = base_per_trick * contract.level as i32;
    if contract.suit == BidSuit::NoTrump {
        contract_points += 10; // first trick is 40, not 30
    }

    if contract.redoubled {
        contract_points *= 4;
    } else if contract.doubled {
        contract_points *= 2;
    }

    // Game bonus: 300 if contract points >= 100, else 50
    let game_bonus = if contract_points >= 100 { 300 } else { 50 };

    // Slam bonus
    let slam_bonus = match contract.level {
        6 => 500,
        7 => 1000,
        _ => 0,
    };

    // Insult bonus (for making doubled/redoubled contracts)
    let insult_bonus = if contract.redoubled {
        100
    } else if contract.doubled {
        50
    } else {
        0
    };

    // Overtrick points
    let overtrick_points = if contract.redoubled {
        overtricks as i32 * 200
    } else if contract.doubled {
        overtricks as i32 * 100
    } else {
        overtricks as i32 * base_per_trick
    };

    let total = contract_points + overtrick_points + game_bonus + slam_bonus + insult_bonus;

    Score::Made {
        contract_points,
        overtrick_points,
        game_bonus,
        slam_bonus,
        insult_bonus,
        total,
    }
}

fn score_defeated(contract: &Contract, undertricks: u8) -> Score {
    // Not vulnerable undertrick penalties
    let penalty = if contract.redoubled {
        redoubled_undertricks(undertricks)
    } else if contract.doubled {
        doubled_undertricks(undertricks)
    } else {
        undertricks as i32 * 50
    };

    Score::Defeated {
        undertricks,
        penalty: -penalty,
    }
}

fn doubled_undertricks(n: u8) -> i32 {
    // Not vulnerable, doubled: 1st=100, 2nd-3rd=200 each, 4th+=300 each
    let mut total = 0;
    for i in 1..=n {
        total += match i {
            1 => 100,
            2 | 3 => 200,
            _ => 300,
        };
    }
    total
}

fn redoubled_undertricks(n: u8) -> i32 {
    // Not vulnerable, redoubled: 1st=200, 2nd-3rd=400 each, 4th+=600 each
    let mut total = 0;
    for i in 1..=n {
        total += match i {
            1 => 200,
            2 | 3 => 400,
            _ => 600,
        };
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Seat;

    fn make_contract(level: u8, suit: BidSuit, doubled: bool, redoubled: bool) -> Contract {
        Contract {
            level,
            suit,
            doubled,
            redoubled,
            declarer: Seat::South,
            dummy: Seat::North,
        }
    }

    // Part-score contracts
    #[test]
    fn test_1nt_made_exact() {
        let c = make_contract(1, BidSuit::NoTrump, false, false);
        let score = calculate_score(&c, 7);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 40,
                overtrick_points: 0,
                game_bonus: 50,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 90,
            }
        );
    }

    #[test]
    fn test_2h_made_exact() {
        let c = make_contract(2, BidSuit::Hearts, false, false);
        let score = calculate_score(&c, 8);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 60,
                overtrick_points: 0,
                game_bonus: 50,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 110,
            }
        );
    }

    #[test]
    fn test_1c_made_with_overtrick() {
        let c = make_contract(1, BidSuit::Clubs, false, false);
        let score = calculate_score(&c, 9); // 2 overtricks
        assert_eq!(
            score,
            Score::Made {
                contract_points: 20,
                overtrick_points: 40,
                game_bonus: 50,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 110,
            }
        );
    }

    // Game contracts
    #[test]
    fn test_3nt_made_exact() {
        let c = make_contract(3, BidSuit::NoTrump, false, false);
        let score = calculate_score(&c, 9);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 100,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 400,
            }
        );
    }

    #[test]
    fn test_4h_made_exact() {
        let c = make_contract(4, BidSuit::Hearts, false, false);
        let score = calculate_score(&c, 10);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 120,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 420,
            }
        );
    }

    #[test]
    fn test_4s_made_with_overtrick() {
        let c = make_contract(4, BidSuit::Spades, false, false);
        let score = calculate_score(&c, 11); // 1 overtrick
        assert_eq!(
            score,
            Score::Made {
                contract_points: 120,
                overtrick_points: 30,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 450,
            }
        );
    }

    #[test]
    fn test_5c_made_exact() {
        let c = make_contract(5, BidSuit::Clubs, false, false);
        let score = calculate_score(&c, 11);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 100,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 400,
            }
        );
    }

    #[test]
    fn test_5d_made_exact() {
        let c = make_contract(5, BidSuit::Diamonds, false, false);
        let score = calculate_score(&c, 11);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 100,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 400,
            }
        );
    }

    // Slam contracts
    #[test]
    fn test_6h_small_slam() {
        let c = make_contract(6, BidSuit::Hearts, false, false);
        let score = calculate_score(&c, 12);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 180,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 500,
                insult_bonus: 0,
                total: 980,
            }
        );
    }

    #[test]
    fn test_7nt_grand_slam() {
        let c = make_contract(7, BidSuit::NoTrump, false, false);
        let score = calculate_score(&c, 13);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 220,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 1000,
                insult_bonus: 0,
                total: 1520,
            }
        );
    }

    // Doubled contracts
    #[test]
    fn test_2s_doubled_made_exact() {
        // 2S doubled: contract pts = 60*2=120 (game!), game bonus 300, insult 50
        let c = make_contract(2, BidSuit::Spades, true, false);
        let score = calculate_score(&c, 8);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 120,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 50,
                total: 470,
            }
        );
    }

    #[test]
    fn test_3h_doubled_with_overtrick() {
        // 3H doubled: contract pts = 90*2=180, game bonus 300, insult 50, overtrick 100
        let c = make_contract(3, BidSuit::Hearts, true, false);
        let score = calculate_score(&c, 10); // 1 overtrick
        assert_eq!(
            score,
            Score::Made {
                contract_points: 180,
                overtrick_points: 100,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 50,
                total: 630,
            }
        );
    }

    // Redoubled contracts
    #[test]
    fn test_1nt_redoubled_made() {
        // 1NT redoubled: contract pts = 40*4=160 (game!), game bonus 300, insult 100
        let c = make_contract(1, BidSuit::NoTrump, false, true);
        let score = calculate_score(&c, 7);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 160,
                overtrick_points: 0,
                game_bonus: 300,
                slam_bonus: 0,
                insult_bonus: 100,
                total: 560,
            }
        );
    }

    #[test]
    fn test_1c_redoubled_with_overtricks() {
        // 1C redoubled: contract pts = 20*4=80, part-score 50, insult 100, overtricks 200 each
        let c = make_contract(1, BidSuit::Clubs, false, true);
        let score = calculate_score(&c, 9); // 2 overtricks
        assert_eq!(
            score,
            Score::Made {
                contract_points: 80,
                overtrick_points: 400,
                game_bonus: 50,
                slam_bonus: 0,
                insult_bonus: 100,
                total: 630,
            }
        );
    }

    // Undertricks - undoubled
    #[test]
    fn test_down_1_undoubled() {
        let c = make_contract(4, BidSuit::Spades, false, false);
        let score = calculate_score(&c, 9); // down 1
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 1,
                penalty: -50,
            }
        );
    }

    #[test]
    fn test_down_3_undoubled() {
        let c = make_contract(4, BidSuit::Spades, false, false);
        let score = calculate_score(&c, 7); // down 3
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 3,
                penalty: -150,
            }
        );
    }

    // Undertricks - doubled
    #[test]
    fn test_down_1_doubled() {
        let c = make_contract(4, BidSuit::Spades, true, false);
        let score = calculate_score(&c, 9); // down 1
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 1,
                penalty: -100,
            }
        );
    }

    #[test]
    fn test_down_2_doubled() {
        let c = make_contract(4, BidSuit::Spades, true, false);
        let score = calculate_score(&c, 8); // down 2
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 2,
                penalty: -300,
            }
        );
    }

    #[test]
    fn test_down_3_doubled() {
        let c = make_contract(3, BidSuit::NoTrump, true, false);
        let score = calculate_score(&c, 6); // down 3
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 3,
                penalty: -500,
            }
        );
    }

    #[test]
    fn test_down_4_doubled() {
        let c = make_contract(4, BidSuit::Spades, true, false);
        let score = calculate_score(&c, 6); // down 4
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 4,
                penalty: -800,
            }
        );
    }

    #[test]
    fn test_down_5_doubled() {
        let c = make_contract(4, BidSuit::Hearts, true, false);
        let score = calculate_score(&c, 5); // down 5
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 5,
                penalty: -1100,
            }
        );
    }

    // Undertricks - redoubled
    #[test]
    fn test_down_1_redoubled() {
        let c = make_contract(4, BidSuit::Spades, false, true);
        let score = calculate_score(&c, 9); // down 1
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 1,
                penalty: -200,
            }
        );
    }

    #[test]
    fn test_down_3_redoubled() {
        let c = make_contract(3, BidSuit::NoTrump, false, true);
        let score = calculate_score(&c, 6); // down 3
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 3,
                penalty: -1000,
            }
        );
    }

    #[test]
    fn test_down_4_redoubled() {
        let c = make_contract(4, BidSuit::Spades, false, true);
        let score = calculate_score(&c, 6); // down 4
        assert_eq!(
            score,
            Score::Defeated {
                undertricks: 4,
                penalty: -1600,
            }
        );
    }

    // Edge cases
    #[test]
    fn test_2nt_made_exact() {
        // 2NT: 40+30 = 70, part-score
        let c = make_contract(2, BidSuit::NoTrump, false, false);
        let score = calculate_score(&c, 8);
        assert_eq!(
            score,
            Score::Made {
                contract_points: 70,
                overtrick_points: 0,
                game_bonus: 50,
                slam_bonus: 0,
                insult_bonus: 0,
                total: 120,
            }
        );
    }

    #[test]
    fn test_6s_small_slam_with_overtrick() {
        let c = make_contract(6, BidSuit::Spades, false, false);
        let score = calculate_score(&c, 13); // grand slam in tricks but bid small slam
        assert_eq!(
            score,
            Score::Made {
                contract_points: 180,
                overtrick_points: 30,
                game_bonus: 300,
                slam_bonus: 500,
                insult_bonus: 0,
                total: 1010,
            }
        );
    }

    #[test]
    fn test_score_total_points() {
        let made = Score::Made {
            contract_points: 120,
            overtrick_points: 0,
            game_bonus: 300,
            slam_bonus: 0,
            insult_bonus: 0,
            total: 420,
        };
        assert_eq!(made.total_points(), 420);

        let defeated = Score::Defeated {
            undertricks: 2,
            penalty: -300,
        };
        assert_eq!(defeated.total_points(), -300);

        assert_eq!(Score::PassedOut.total_points(), 0);
    }
}
