mod utils;

use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use rand::prelude::*;
use std::collections::HashMap;

type Player = usize;
type GuessNum = usize;
type Tree = HashMap<Game, ActionScores>;

const MIN_PLAYOUTS: usize = 2048;

fn cartesian_product(nrows: usize, ncols: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..nrows).flat_map(move |row| (0..ncols).map(move |col| (row, col)))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Cell {
    Empty,
    CrossedOut,
    Guess(Player, GuessNum),
}

impl std::fmt::Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Cell::Empty => write!(f, "   "),
            Cell::CrossedOut => write!(f, " X "),
            Cell::Guess(player, guess_num) => write!(f, "{} {}", player, guess_num),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct Action {
    row: usize,
    col: usize,
    cell: Cell,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Game {
    active_player: Player,
    board: Vec<Cell>,
    nrows: usize,
    ncols: usize,
}

impl Game {
    pub fn new(nrows: usize, ncols: usize) -> Game {
        Game {
            active_player: 0,
            board: vec![Cell::Empty; nrows * ncols],
            nrows,
            ncols,
        }
    }

    pub fn is_finished(&self) -> bool {
        cartesian_product(self.nrows, self.ncols)
            .all(|(row, col)| self.get_cell(row, col) != &Cell::Empty)
    }

    pub fn empty_cells(&self) -> usize {
        cartesian_product(self.nrows, self.ncols)
            .filter(|&(row, col)| self.get_cell(row, col) == &Cell::Empty)
            .count()
    }

    pub fn get_scores(&self) -> [usize; 2] {
        let mut scores = [0, 0];
        for row in 0..self.nrows {
            let mut is_full = true;
            let mut num_guesses = 0;
            for col in 0..self.ncols {
                match self.get_cell(row, col) {
                    Cell::Empty => is_full = false,
                    Cell::Guess(_, _) => num_guesses += 1,
                    Cell::CrossedOut => (),
                };
            }
            if !is_full {
                continue;
            }
            for col in 0..self.ncols {
                if let &Cell::Guess(player, guess_num) = self.get_cell(row, col) {
                    if guess_num == num_guesses {
                        scores[player] += num_guesses;
                    }
                }
            }
        }
        for col in 0..self.ncols {
            let mut is_full = true;
            let mut num_guesses = 0;
            for row in 0..self.nrows {
                match self.get_cell(row, col) {
                    Cell::Empty => is_full = false,
                    Cell::Guess(_, _) => num_guesses += 1,
                    Cell::CrossedOut => (),
                };
            }
            if !is_full {
                continue;
            }
            for row in 0..self.nrows {
                if let &Cell::Guess(player, guess_num) = self.get_cell(row, col) {
                    if guess_num == num_guesses {
                        scores[player] += num_guesses;
                    }
                }
            }
        }
        scores
    }

    pub fn get_cell(&self, row: usize, col: usize) -> &Cell {
        &self.board[row * self.ncols + col]
    }

    pub fn set_cell(&mut self, row: usize, col: usize, cell: Cell) {
        self.board[row * self.ncols + col] = cell;
    }

    pub fn is_legal_move(&self, row: usize, col: usize, cell: &Cell) -> Result<(), &'static str> {
        if row >= self.nrows {
            return Err("Row is out of bounds");
        }
        if col >= self.ncols {
            return Err("Column is out of bounds");
        }

        if *self.get_cell(row, col) != Cell::Empty {
            return Err("Cannot place on a non-empty square");
        }
        match *cell {
            Cell::Empty => Err("Cannot erase a square"),
            Cell::CrossedOut => Ok(()),
            Cell::Guess(player, guess_num) => {
                if player != self.active_player {
                    return Err("Cannot place a guess for your opponent");
                }
                if guess_num == 0 {
                    return Err("Cannot guess 0");
                }
                if guess_num > self.nrows.max(self.ncols) {
                    return Err("Guess cannot be larger than both the grid's width and height");
                }
                for (other_row, other_col) in cartesian_product(self.nrows, self.ncols) {
                    if other_row == row || other_col == col {
                        match self.get_cell(other_row, other_col) {
                            &Cell::Guess(_, other_guess_num) => {
                                if guess_num == other_guess_num {
                                    return Err("Only one of each guess value per row/column");
                                }
                            }
                            _ => (),
                        }
                    }
                }
                Ok(())
            }
        }
    }

    pub fn get_legal_actions<'a>(&'a self) -> Box<dyn Iterator<Item = Action> + 'a> {
        let max_guess = self.nrows.max(self.ncols);
        Box::new(
            (0..self.nrows)
                .flat_map(move |row| (0..self.ncols).map(move |col| (row, col)))
                .filter(move |&(row, col)| self.get_cell(row, col) == &Cell::Empty)
                .flat_map(move |(row, col)| {
                    [Cell::CrossedOut]
                        .iter()
                        .cloned()
                        .chain(
                            (1..=max_guess)
                                .map(move |guess_num| Cell::Guess(self.active_player, guess_num)),
                        )
                        .filter(move |&cell| match self.is_legal_move(row, col, &cell) {
                            Ok(()) => true,
                            Err(_) => false,
                        })
                        .map(move |cell| Action { row, col, cell })
                }),
        )
    }

    pub fn place(&mut self, row: usize, col: usize, cell: Cell) -> Result<(), IllegalMoveError> {
        if let Err(msg) = self.is_legal_move(row, col, &cell) {
            return Err(IllegalMoveError {
                game: self.clone(),
                row,
                col,
                cell,
                msg,
            });
        }

        match cell {
            Cell::Empty => panic!("Cannot place an empty cell!"),
            Cell::CrossedOut => {
                self.set_cell(row, col, cell);
                self.active_player = 1 - self.active_player;
            }
            Cell::Guess(_, _) => {
                self.set_cell(row, col, cell);
                self.active_player = 1 - self.active_player;
                for (other_row, other_col) in cartesian_product(self.nrows, self.ncols) {
                    let should_cross_out = |other_row, other_col| {
                        if other_row != row && other_col != col {
                            return false;
                        }
                        let other_cell = self.get_cell(other_row, other_col);
                        if other_cell != &Cell::Empty {
                            return false;
                        }
                        for guess_num in 1..=self.nrows.max(self.ncols) {
                            match self.is_legal_move(
                                other_row,
                                other_col,
                                &Cell::Guess(self.active_player, guess_num),
                            ) {
                                Ok(()) => return false,
                                Err(_) => (),
                            }
                        }
                        return true;
                    };
                    if should_cross_out(other_row, other_col) {
                        self.set_cell(other_row, other_col, Cell::CrossedOut);
                    }
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for row in 0..self.nrows {
            write!(f, "|")?;
            for col in 0..self.ncols {
                write!(f, "{}|", self.get_cell(row, col))?;
            }
            write!(f, "\n")?;
        }
        let scores = self.get_scores();
        write!(f, "Scores: {}, {}\n", scores[0], scores[1])?;
        if self.is_finished() {
            if scores[0] < scores[1] {
                write!(f, "Player 1 wins.")
            } else if scores[1] < scores[0] {
                write!(f, "Player 0 wins.")
            } else {
                write!(f, "Draw!")
            }
        } else {
            write!(f, "Player {} to move.", self.active_player)
        }
    }
}

#[derive(Clone, Debug)]
pub struct IllegalMoveError {
    game: Game,
    row: usize,
    col: usize,
    cell: Cell,
    msg: &'static str,
}

impl std::fmt::Display for IllegalMoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Cannot perform {:?} at ({}, {}) on {:?}: {}",
            self.cell, self.row, self.col, self.game, self.msg
        )
    }
}

impl std::error::Error for IllegalMoveError {}

struct ActionScores {
    pub visits: HashMap<Action, (u64, f64)>,
    pub available_actions: Vec<Action>,
}

impl ActionScores {
    pub fn new(game: &Game) -> Self {
        Self {
            visits: HashMap::new(),
            available_actions: game.get_legal_actions().collect(),
        }
    }

    pub fn mark_visit(&mut self, edge: Action, reward: f64) {
        let (ref mut visits, ref mut rewards) = self.visits.entry(edge).or_insert((0, 0.0));
        *visits += 1;
        *rewards += reward;
    }

    pub fn get_visit(&self, edge: Action) -> (u64, f64) {
        if let Some(&ans) = self.visits.get(&edge) {
            ans
        } else {
            (0, 0.0)
        }
    }
}

fn choose_child(tally: &ActionScores, actions: &[Action], rng: &mut impl rand::Rng) -> Action {
    let total_visits = actions.iter().map(|&x| tally.get_visit(x).0).sum::<u64>();

    let mut choice = None;
    let mut num_optimal: u32 = 0;
    let mut best_so_far: f64 = std::f64::NEG_INFINITY;
    for &action in actions {
        let score = {
            let (child_visits, sum_rewards) = tally.get_visit(action);
            // https://www.researchgate.net/publication/235985858_A_Survey_of_Monte_Carlo_Tree_Search_Methods
            if child_visits == 0 {
                std::f64::INFINITY
            } else {
                let explore_term = (2.0 * (total_visits as f64).ln() / child_visits as f64).sqrt();
                let exploit_term = (sum_rewards + 1.0) / (child_visits as f64 + 2.0);
                explore_term + exploit_term
            }
        };
        if score > best_so_far {
            choice = Some(action);
            num_optimal = 1;
            best_so_far = score;
        } else if (score - best_so_far).abs() < std::f64::EPSILON {
            num_optimal += 1;
            if rng.gen_bool(1.0 / num_optimal as f64) {
                choice = Some(action);
            }
        }
    }
    choice.unwrap()
}

fn playout(root: &Game, tree: &mut Tree) {
    if root.is_finished() {
        return;
    }
    let mut path = vec![];
    let mut node = root.clone();
    let mut rng = rand::rngs::OsRng {};
    while let Some(tally) = tree.get(&node) {
        if tally.available_actions.is_empty() {
            break;
        }
        let action = choose_child(tally, tally.available_actions.as_slice(), &mut rng);
        path.push((node.clone(), action));
        node.place(action.row, action.col, action.cell).unwrap();
    }

    tree.entry(node.clone())
        .or_insert_with(|| ActionScores::new(&node));

    loop {
        let available_moves = node.get_legal_actions().collect::<Vec<Action>>();
        if available_moves.is_empty() {
            break;
        }
        let &action = available_moves.choose(&mut rng).unwrap();
        path.push((node.clone(), action));
        node.place(action.row, action.col, action.cell).unwrap();
    }

    assert!(path[0].0 == *root);
    assert!(node.is_finished());
    let scores = node.get_scores();
    for (backprop_node, action) in path {
        let p = backprop_node.active_player;
        if let Some(action_scores) = tree.get_mut(&backprop_node) {
            let score = match scores[p].cmp(&scores[1 - p]) {
                std::cmp::Ordering::Greater => 1.0,
                std::cmp::Ordering::Equal => 0.0,
                std::cmp::Ordering::Less => -1.0,
            };
            action_scores.mark_visit(action, score);
        } else {
            break;
        }
    }
    assert!(tree.get(root).is_some())
}

pub struct MCTSBot {
    pub root: Game,
    pub me: Player,
    tree: Tree,
}

impl MCTSBot {
    pub fn new(root: Game, me: Player) -> Self {
        let bot = MCTSBot {
            root,
            me,
            tree: HashMap::new(),
        };
        bot
    }

    pub fn get_best_action(&self) -> Option<(Action, (u64, f64))> {
        if self.root.is_finished() {
            return None;
        }
        self.tree
            .get(&self.root)
            .map(|action_scores| {
                action_scores
                    .visits
                    .iter()
                    .max_by(|(_, &(visits1, score1)), (_, &(visits2, score2))| {
                        (score1 / visits1 as f64)
                            .partial_cmp(&(score2 / visits2 as f64))
                            .unwrap()
                    })
                    .map(|(&action, &scores)| (action, scores))
            })
            .flatten()
    }

    /// Tell the bot about the new game state
    pub fn update(&mut self, game: Game) {
        self.tree
            .retain(|g, _| g.empty_cells() <= game.empty_cells());
        self.root = game;
    }

    pub fn playout(&mut self) {
        playout(&self.root, &mut self.tree);
    }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Debug)]
pub struct WasmAction {
    pub row: usize,
    pub col: usize,
    pub guess: usize,
}

impl From<Action> for WasmAction {
    fn from(action: Action) -> Self {
        Self {
            row: action.row,
            col: action.col,
            guess: match action.cell {
                Cell::Empty => panic!("We can't move by placing an empty cell"),
                Cell::CrossedOut => 0,
                Cell::Guess(_, guess) => guess,
            },
        }
    }
}

#[wasm_bindgen]
#[derive(Copy, Clone, Debug)]
pub struct WasmEdge {
    pub action: WasmAction,
    pub visits: u64,
    pub score: f64,
}

#[wasm_bindgen]
pub struct WasmCell {
    pub player: Option<usize>,
    /**
     * None for an empty cell, 0 for a crossed-out cell, otherwise a number between 1 and max(nrows, ncols)
     */
    pub guess: Option<usize>,
}

#[wasm_bindgen]
pub struct WasmBot {
    // Opaque pointers for JS interop
    pub bot: *mut MCTSBot,
}

impl Drop for WasmBot {
    fn drop(&mut self) {
        // Manually free opaque pointers
        let _ = unsafe { Box::from_raw(self.bot) };
    }
}

#[wasm_bindgen]
impl WasmBot {
    pub fn new(nrows: usize, ncols: usize, me: usize) -> Self {
        utils::set_panic_hook();
        Self {
            bot: Box::into_raw(Box::new(MCTSBot::new(Game::new(nrows, ncols), me))),
        }
    }

    pub fn is_finished(&self) -> bool {
        let bot = unsafe { &*self.bot };
        bot.root.is_finished()
    }

    pub fn get_scores(&self) -> Vec<usize> {
        let bot = unsafe { &*self.bot };
        bot.root.get_scores().iter().cloned().collect()
    }

    pub fn get_cell(&self, row: usize, col: usize) -> Result<WasmCell, JsValue> {
        let bot = unsafe { &*self.bot };
        let game = &bot.root;
        if row >= game.nrows || col >= game.ncols {
            return Err(JsValue::from_str(&format!(
                "Cannot access {}, {}",
                row, col
            )));
        }
        let cell = game.get_cell(row, col);
        Ok(match cell {
            Cell::CrossedOut => WasmCell {
                player: None,
                guess: Some(0),
            },
            Cell::Empty => WasmCell {
                player: None,
                guess: None,
            },
            Cell::Guess(player, guess) => WasmCell {
                player: Some(*player),
                guess: Some(*guess),
            },
        })
    }

    pub fn place(&mut self, row: usize, col: usize, guess: usize) -> Result<(), JsValue> {
        let bot = unsafe { &mut *self.bot };
        let mut game = bot.root.clone();
        let cell = if guess > 0 {
            Cell::Guess(game.active_player, guess)
        } else {
            Cell::CrossedOut
        };
        match game.place(row, col, cell) {
            Ok(ok) => {
                bot.update(game);
                Ok(ok)
            }
            Err(err) => Err(JsValue::from_str(format!("{:?}", err).as_str())),
        }
    }

    pub fn playout(&mut self) {
        let bot = unsafe { &mut *self.bot };
        bot.playout();
    }

    pub fn get_active_player(&self) -> usize {
        let bot = unsafe { &mut *self.bot };
        return bot.root.active_player;
    }

    pub fn get_best_action(&mut self) -> Option<WasmEdge> {
        let bot = unsafe { &mut *self.bot };
        if bot.root.is_finished() {
            return None;
        }
        for _ in 0..MIN_PLAYOUTS {
            bot.playout();
        }
        let action = bot.get_best_action();
        match action {
            None => None,
            Some((action, (visits, score))) => Some(WasmEdge {
                action: action.into(),
                visits,
                score,
            }),
        }
    }
}
