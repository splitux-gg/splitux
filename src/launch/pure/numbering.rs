//! Per-game (unit) instance numbering.
//!
//! Multi-game mode packs several games' instances into one flat `instances`
//! list, tagged by `Instance.game`. A handful of behaviors are **per-unit**, not
//! global, and must be derived from that tag:
//!   - `$INSTANCENUM` / `$INSTANCECOUNT` handler-arg substitution,
//!   - goldberg's "first instance reports the REAL save's steam id" rule.
//!
//! For a single-game launch every tag is `0`, so the per-game number equals the
//! global index and the per-game count equals the total — i.e. byte-identical to
//! the pre-multi-game behavior.

/// Given each instance's game tag (in launch order), return two parallel
/// vectors:
///   - `nums[i]`   = this instance's 0-based position **within its own game**,
///   - `counts[i]` = the total number of instances sharing instance `i`'s game.
///
/// `nums[i] == 0` marks the first instance of a game (the goldberg real-save
/// owner). For a uniform `[0, 0, …]` input this yields `nums == [0, 1, 2, …]`
/// and `counts == [n, n, …]`, exactly the old global `i` / `instances.len()`.
pub fn per_game_instance_numbering(games: &[usize]) -> (Vec<usize>, Vec<usize>) {
    use std::collections::HashMap;

    let mut totals: HashMap<usize, usize> = HashMap::new();
    for &g in games {
        *totals.entry(g).or_insert(0) += 1;
    }

    let mut seen: HashMap<usize, usize> = HashMap::new();
    let mut nums = Vec::with_capacity(games.len());
    let mut counts = Vec::with_capacity(games.len());
    for &g in games {
        let n = seen.entry(g).or_insert(0);
        nums.push(*n);
        *n += 1;
        counts.push(totals[&g]);
    }
    (nums, counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_game_matches_global_index_and_len() {
        // The single-game invariant: per-game numbering == global i / len.
        for n in 1..=8usize {
            let games = vec![0usize; n];
            let (nums, counts) = per_game_instance_numbering(&games);
            assert_eq!(nums, (0..n).collect::<Vec<_>>(), "nums for n={n}");
            assert_eq!(counts, vec![n; n], "counts for n={n}");
        }
    }

    #[test]
    fn two_games_number_independently() {
        // game 0 has 2 instances, game 1 has 3, interleaved in launch order.
        let games = [0, 1, 0, 1, 1];
        let (nums, counts) = per_game_instance_numbering(&games);
        assert_eq!(nums, vec![0, 0, 1, 1, 2]);
        assert_eq!(counts, vec![2, 3, 2, 3, 3]);
    }

    #[test]
    fn first_of_each_game_is_marked_zero() {
        let games = [2, 2, 0, 1, 0];
        let (nums, _) = per_game_instance_numbering(&games);
        // The first time each game id appears, its num is 0.
        assert_eq!(nums[0], 0); // first game-2
        assert_eq!(nums[1], 1); // second game-2
        assert_eq!(nums[2], 0); // first game-0
        assert_eq!(nums[3], 0); // first game-1
        assert_eq!(nums[4], 1); // second game-0
    }

    #[test]
    fn empty_input_is_empty() {
        let (nums, counts) = per_game_instance_numbering(&[]);
        assert!(nums.is_empty());
        assert!(counts.is_empty());
    }
}
