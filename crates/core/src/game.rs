//! Game state, actions and rules.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::cards::{card_def, ActionKind, CardId, CardKind, Color, PACK_SIZE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub u8);

/// Identity of one property set on the table. Unique within a game and never reused.
/// A set keeps its id when a Deal Breaker moves it to another player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SetId(pub u32);

pub const MIN_PLAYERS: usize = 2;
/// Two packs (212 cards) can deal a 5-card hand to at most 42 players.
pub const MAX_PLAYERS: usize = 2 * PACK_SIZE as usize / HAND_DEAL;
pub const HAND_DEAL: usize = 5;
pub const HAND_LIMIT: usize = 7;
pub const PLAYS_PER_TURN: u8 = 3;
pub const SETS_TO_WIN: usize = 3;
const LOG_TAIL: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameConfig {
    pub num_players: usize,
    pub seed: u64,
}

impl GameConfig {
    pub fn new(num_players: usize, seed: u64) -> Self {
        Self { num_players, seed }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Bank {
        card: CardId,
    },
    /// `set: Some` joins that incomplete set of `color`; `None` starts a new set.
    PlayProperty {
        card: CardId,
        color: Color,
        #[serde(default)]
        set: Option<SetId>,
    },
    PlayPassGo {
        card: CardId,
    },
    PlayBirthday {
        card: CardId,
    },
    PlayDebtCollector {
        card: CardId,
        target: PlayerId,
    },
    /// `target` is set only for the any-color rent. `doubles` are Double the Rent cards from hand.
    PlayRent {
        card: CardId,
        set: SetId,
        target: Option<PlayerId>,
        doubles: Vec<CardId>,
    },
    PlaySlyDeal {
        card: CardId,
        take: CardId,
    },
    PlayForcedDeal {
        card: CardId,
        give: CardId,
        take: CardId,
    },
    PlayDealBreaker {
        card: CardId,
        set: SetId,
    },
    PlayHouse {
        card: CardId,
        set: SetId,
    },
    PlayHotel {
        card: CardId,
        set: SetId,
    },
    /// Free rearrangement on your own turn. Wilds may change color; any property may move to
    /// another incomplete set of its color (`set: Some`) or to a new set (`None`).
    MoveProperty {
        card: CardId,
        color: Color,
        #[serde(default)]
        set: Option<SetId>,
    },
    EndTurn,
    Discard {
        card: CardId,
    },
    JustSayNo {
        card: CardId,
    },
    Accept,
    PayCard {
        card: CardId,
    },
}

/// What an action card does to each target once it is not cancelled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Effect {
    Charge { amount: u32 },
    SlyDeal { take: CardId },
    ForcedDeal { give: CardId, take: CardId },
    DealBreaker { set: SetId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Phase {
    Play {
        player: PlayerId,
        plays_left: u8,
    },
    /// `responder` may accept or play Just Say No. The window opens whether or not they hold one.
    Respond {
        actor: PlayerId,
        target: PlayerId,
        responder: PlayerId,
        card: CardId,
        effect: Effect,
        just_say_no_count: u8,
    },
    Pay {
        payer: PlayerId,
        to: PlayerId,
        owed: u32,
        paid: u32,
    },
    Discard {
        player: PlayerId,
        excess: usize,
    },
    GameOver {
        winner: PlayerId,
    },
}

/// Public game events. They never contain hidden card ids.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    TurnStarted {
        player: PlayerId,
        turn: u32,
    },
    Drew {
        player: PlayerId,
        count: usize,
    },
    DrawPileReshuffled {
        count: usize,
    },
    Banked {
        player: PlayerId,
        card: CardId,
    },
    PropertyPlayed {
        player: PlayerId,
        card: CardId,
        color: Color,
        set: SetId,
    },
    PropertyMoved {
        player: PlayerId,
        card: CardId,
        color: Color,
        set: SetId,
    },
    PassGo {
        player: PlayerId,
        card: CardId,
    },
    ActionPlayed {
        player: PlayerId,
        card: CardId,
        doubles: Vec<CardId>,
        effect: Effect,
        targets: Vec<PlayerId>,
    },
    BuildingPlaced {
        player: PlayerId,
        card: CardId,
        set: SetId,
    },
    JustSayNo {
        player: PlayerId,
        card: CardId,
    },
    Accepted {
        player: PlayerId,
    },
    ActionCancelled {
        actor: PlayerId,
        target: PlayerId,
    },
    PropertyTaken {
        from: PlayerId,
        to: PlayerId,
        card: CardId,
    },
    SetTaken {
        from: PlayerId,
        to: PlayerId,
        set: SetId,
    },
    Paid {
        from: PlayerId,
        to: PlayerId,
        card: CardId,
    },
    PaymentFinished {
        from: PlayerId,
        to: PlayerId,
        owed: u32,
        paid: u32,
    },
    TurnEnded {
        player: PlayerId,
    },
    DiscardedToDrawPile {
        player: PlayerId,
    },
    GameWon {
        winner: PlayerId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertySet {
    pub id: SetId,
    pub color: Color,
    pub cards: Vec<CardId>,
    pub house: Option<CardId>,
    pub hotel: Option<CardId>,
}

impl PropertySet {
    pub fn is_complete(&self) -> bool {
        self.cards.len() >= self.color.set_size()
    }

    /// Rent this set charges before Double the Rent. Buildings count only while the set is full.
    pub fn rent(&self) -> u32 {
        if self.cards.is_empty() {
            return 0;
        }
        let ladder = self.color.rent_ladder();
        let mut rent = ladder[self.cards.len().min(ladder.len()) - 1] as u32;
        if self.is_complete() {
            rent += self.house.map_or(0, |c| card_def(c).value as u32);
            rent += self.hotel.map_or(0, |c| card_def(c).value as u32);
        }
        rent
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PlayerState {
    hand: Vec<CardId>,
    bank: Vec<CardId>,
    sets: Vec<PropertySet>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Step {
    Respond {
        responder: PlayerId,
        just_say_no_count: u8,
    },
    Pay {
        owed: u32,
        paid: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Resolution {
    actor: PlayerId,
    card: CardId,
    effect: Effect,
    target: PlayerId,
    /// Targets still waiting, in seat order after the actor.
    queue: Vec<PlayerId>,
    step: Step,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Stage {
    Play,
    Resolving(Resolution),
    Discard,
    GameOver,
}

/// Full, authoritative game state. It contains hidden information (hands, draw order, RNG state):
/// serialize it only to trusted storage and show clients `observation` instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    rng: u64,
    /// Top of the pile is the last element.
    draw_pile: Vec<CardId>,
    play_pile: Vec<CardId>,
    players: Vec<PlayerState>,
    current: PlayerId,
    turn: u32,
    plays_left: u8,
    stage: Stage,
    winner: Option<PlayerId>,
    next_set_id: u32,
    log: Vec<Event>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameError {
    InvalidPlayerCount {
        num_players: usize,
        min: usize,
        max: usize,
    },
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::InvalidPlayerCount {
                num_players,
                min,
                max,
            } => {
                write!(
                    f,
                    "invalid player count {num_players}: must be {min}..={max}"
                )
            }
        }
    }
}

impl std::error::Error for GameError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionError {
    GameOver,
    UnknownPlayer { player: PlayerId },
    NotYourDecision { player: PlayerId },
    Illegal { action: Action },
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionError::GameOver => write!(f, "the game is over"),
            ActionError::UnknownPlayer { player } => write!(f, "unknown player {}", player.0),
            ActionError::NotYourDecision { player } => {
                write!(f, "player {} has no decision to make now", player.0)
            }
            ActionError::Illegal { action } => write!(f, "illegal action: {action:?}"),
        }
    }
}

impl std::error::Error for ActionError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicPlayer {
    pub id: PlayerId,
    pub hand_count: usize,
    pub bank: Vec<CardId>,
    pub bank_value: u32,
    pub properties: Vec<PublicSet>,
    /// Distinct colors with a full set. Three wins on the owner's turn.
    pub complete_colors: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicSet {
    pub id: SetId,
    pub color: Color,
    pub cards: Vec<CardId>,
    pub house: Option<CardId>,
    pub hotel: Option<CardId>,
    pub complete: bool,
    pub rent: u32,
}

/// What one viewer may know. A spectator (`viewer: None`) sees no hand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub viewer: Option<PlayerId>,
    pub hand: Vec<CardId>,
    pub players: Vec<PublicPlayer>,
    pub draw_count: usize,
    /// Played action cards, oldest first.
    pub discard: Vec<CardId>,
    pub current_player: PlayerId,
    pub turn: u32,
    pub phase: Phase,
    pub pending_actors: Vec<PlayerId>,
    pub winner: Option<PlayerId>,
    /// The viewer's legal actions; empty for spectators.
    pub legal_actions: Vec<Action>,
    /// The last 50 public events.
    pub log: Vec<Event>,
}

impl Game {
    pub fn new(config: GameConfig) -> Result<Game, GameError> {
        let n = config.num_players;
        if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&n) {
            return Err(GameError::InvalidPlayerCount {
                num_players: n,
                min: MIN_PLAYERS,
                max: MAX_PLAYERS,
            });
        }
        let packs: u16 = if n >= 6 { 2 } else { 1 };
        let mut game = Game {
            rng: config.seed,
            draw_pile: (0..packs * PACK_SIZE).map(CardId).collect(),
            play_pile: Vec::new(),
            players: vec![
                PlayerState {
                    hand: Vec::new(),
                    bank: Vec::new(),
                    sets: Vec::new()
                };
                n
            ],
            current: PlayerId(0),
            turn: 0,
            plays_left: 0,
            stage: Stage::Play,
            winner: None,
            next_set_id: 0,
            log: Vec::new(),
        };
        let mut pile = std::mem::take(&mut game.draw_pile);
        game.shuffle(&mut pile);
        game.draw_pile = pile;
        for _ in 0..HAND_DEAL {
            for p in 0..n {
                let card = game
                    .draw_pile
                    .pop()
                    .expect("deck holds at least 5 cards per player");
                game.players[p].hand.push(card);
            }
        }
        game.start_turn(PlayerId(0));
        Ok(game)
    }

    pub fn num_players(&self) -> usize {
        self.players.len()
    }

    /// Every card id in this game: 0..106 with one pack (2-5 players), 0..212 with two packs (6+).
    pub fn card_ids(&self) -> impl Iterator<Item = CardId> {
        let packs: u16 = if self.players.len() >= 6 { 2 } else { 1 };
        (0..packs * PACK_SIZE).map(CardId)
    }

    pub fn winner(&self) -> Option<PlayerId> {
        self.winner
    }

    pub fn current_player(&self) -> PlayerId {
        self.current
    }

    pub fn turn(&self) -> u32 {
        self.turn
    }

    /// Full public event log.
    pub fn log(&self) -> &[Event] {
        &self.log
    }

    pub fn phase(&self) -> Phase {
        match &self.stage {
            Stage::Play => Phase::Play {
                player: self.current,
                plays_left: self.plays_left,
            },
            Stage::Discard => Phase::Discard {
                player: self.current,
                excess: self
                    .player(self.current)
                    .hand
                    .len()
                    .saturating_sub(HAND_LIMIT),
            },
            Stage::GameOver => Phase::GameOver {
                winner: self.winner.expect("game over has a winner"),
            },
            Stage::Resolving(r) => match r.step {
                Step::Respond {
                    responder,
                    just_say_no_count,
                } => Phase::Respond {
                    actor: r.actor,
                    target: r.target,
                    responder,
                    card: r.card,
                    effect: r.effect.clone(),
                    just_say_no_count,
                },
                Step::Pay { owed, paid } => Phase::Pay {
                    payer: r.target,
                    to: r.actor,
                    owed,
                    paid,
                },
            },
        }
    }

    /// Players who must decide now. Decisions are serial, so this holds at most one player.
    pub fn pending_actors(&self) -> Vec<PlayerId> {
        self.to_act().into_iter().collect()
    }

    pub fn to_act(&self) -> Option<PlayerId> {
        match &self.stage {
            Stage::Play | Stage::Discard => Some(self.current),
            Stage::GameOver => None,
            Stage::Resolving(r) => Some(match r.step {
                Step::Respond { responder, .. } => responder,
                Step::Pay { .. } => r.target,
            }),
        }
    }

    pub fn observation(&self, viewer: Option<PlayerId>) -> Observation {
        let viewer = viewer.filter(|p| (p.0 as usize) < self.players.len());
        Observation {
            viewer,
            hand: viewer
                .map(|p| self.player(p).hand.clone())
                .unwrap_or_default(),
            players: (0..self.players.len())
                .map(|i| self.public_player(PlayerId(i as u8)))
                .collect(),
            draw_count: self.draw_pile.len(),
            discard: self.play_pile.clone(),
            current_player: self.current,
            turn: self.turn,
            phase: self.phase(),
            pending_actors: self.pending_actors(),
            winner: self.winner,
            legal_actions: viewer.map(|p| self.legal_actions(p)).unwrap_or_default(),
            log: self.log[self.log.len().saturating_sub(LOG_TAIL)..].to_vec(),
        }
    }

    fn public_player(&self, id: PlayerId) -> PublicPlayer {
        let p = self.player(id);
        PublicPlayer {
            id,
            hand_count: p.hand.len(),
            bank: p.bank.clone(),
            bank_value: p.bank.iter().map(|&c| card_def(c).value as u32).sum(),
            properties: p
                .sets
                .iter()
                .map(|s| PublicSet {
                    id: s.id,
                    color: s.color,
                    cards: s.cards.clone(),
                    house: s.house,
                    hotel: s.hotel,
                    complete: s.is_complete(),
                    rent: s.rent(),
                })
                .collect(),
            complete_colors: self.complete_colors(id),
        }
    }

    /// Every legal action for `player` now. Empty when it is not their decision.
    /// Payment and discards are one card per action, so every legal selection is reachable.
    /// For rent, `doubles` lists the lowest-id Double the Rent cards in hand; any other
    /// Double the Rent cards from hand are accepted by `apply` as equivalent.
    pub fn legal_actions(&self, player: PlayerId) -> Vec<Action> {
        if self.to_act() != Some(player) {
            return Vec::new();
        }
        let me = self.player(player);
        match &self.stage {
            Stage::GameOver => Vec::new(),
            Stage::Discard => me
                .hand
                .iter()
                .map(|&card| Action::Discard { card })
                .collect(),
            Stage::Resolving(r) => match r.step {
                Step::Respond { .. } => {
                    let mut actions = vec![Action::Accept];
                    actions.extend(
                        me.hand
                            .iter()
                            .filter(|&&c| card_def(c).action() == Some(ActionKind::JustSayNo))
                            .map(|&card| Action::JustSayNo { card }),
                    );
                    actions
                }
                Step::Pay { .. } => self
                    .payable_cards(player)
                    .into_iter()
                    .map(|card| Action::PayCard { card })
                    .collect(),
            },
            Stage::Play => self.play_actions(player),
        }
    }

    fn play_actions(&self, player: PlayerId) -> Vec<Action> {
        let me = self.player(player);
        let mut actions = Vec::new();
        for from in &me.sets {
            for &card in &from.cards {
                for &color in card_def(card).property_colors() {
                    for to in self.placements(player, color) {
                        let no_op = match to {
                            Some(id) => id == from.id,
                            None => color == from.color && from.cards.len() == 1,
                        };
                        if !no_op {
                            actions.push(Action::MoveProperty {
                                card,
                                color,
                                set: to,
                            });
                        }
                    }
                }
            }
        }
        actions.push(Action::EndTurn);
        if self.plays_left == 0 {
            return actions;
        }
        let others: Vec<PlayerId> = self.seats_after(player);
        let mut doubles: Vec<CardId> = me
            .hand
            .iter()
            .copied()
            .filter(|&c| card_def(c).action() == Some(ActionKind::DoubleTheRent))
            .collect();
        doubles.sort();
        for &card in &me.hand {
            let def = card_def(card);
            if def.bankable() {
                actions.push(Action::Bank { card });
            }
            match def.kind {
                CardKind::Money => {}
                CardKind::Property { .. } | CardKind::Wild { .. } => {
                    for &color in def.property_colors() {
                        for set in self.placements(player, color) {
                            actions.push(Action::PlayProperty { card, color, set });
                        }
                    }
                }
                CardKind::Rent { colors } => {
                    let any_color = colors.len() > 2;
                    let max_doubles = doubles.len().min(self.plays_left as usize - 1);
                    for set in me
                        .sets
                        .iter()
                        .filter(|s| colors.contains(&s.color) && !s.cards.is_empty())
                    {
                        let targets: Vec<Option<PlayerId>> = if any_color {
                            others.iter().map(|&p| Some(p)).collect()
                        } else {
                            vec![None]
                        };
                        for target in targets {
                            for k in 0..=max_doubles {
                                actions.push(Action::PlayRent {
                                    card,
                                    set: set.id,
                                    target,
                                    doubles: doubles[..k].to_vec(),
                                });
                            }
                        }
                    }
                }
                CardKind::Action { action } => match action {
                    ActionKind::PassGo => actions.push(Action::PlayPassGo { card }),
                    ActionKind::ItsMyBirthday => actions.push(Action::PlayBirthday { card }),
                    ActionKind::DebtCollector => actions.extend(
                        others
                            .iter()
                            .map(|&target| Action::PlayDebtCollector { card, target }),
                    ),
                    ActionKind::SlyDeal => {
                        for &other in &others {
                            for take in self.unprotected_properties(other) {
                                actions.push(Action::PlaySlyDeal { card, take });
                            }
                        }
                    }
                    ActionKind::ForcedDeal => {
                        let gives = self.unprotected_properties(player);
                        for &other in &others {
                            for take in self.unprotected_properties(other) {
                                for &give in &gives {
                                    actions.push(Action::PlayForcedDeal { card, give, take });
                                }
                            }
                        }
                    }
                    ActionKind::DealBreaker => {
                        for &other in &others {
                            for set in self.player(other).sets.iter().filter(|s| s.is_complete()) {
                                actions.push(Action::PlayDealBreaker { card, set: set.id });
                            }
                        }
                    }
                    ActionKind::House => {
                        for set in me.sets.iter().filter(|s| {
                            s.is_complete() && s.color.allows_buildings() && s.house.is_none()
                        }) {
                            actions.push(Action::PlayHouse { card, set: set.id });
                        }
                    }
                    ActionKind::Hotel => {
                        for set in me
                            .sets
                            .iter()
                            .filter(|s| s.is_complete() && s.house.is_some() && s.hotel.is_none())
                        {
                            actions.push(Action::PlayHotel { card, set: set.id });
                        }
                    }
                    ActionKind::JustSayNo | ActionKind::DoubleTheRent => {}
                },
            }
        }
        actions
    }

    /// Apply `action` for `player`. On error the game is unchanged.
    pub fn apply(&mut self, player: PlayerId, action: Action) -> Result<Vec<Event>, ActionError> {
        if self.winner.is_some() {
            return Err(ActionError::GameOver);
        }
        if player.0 as usize >= self.players.len() {
            return Err(ActionError::UnknownPlayer { player });
        }
        if self.to_act() != Some(player) {
            return Err(ActionError::NotYourDecision { player });
        }
        let canonical =
            self.canonical_doubles(player, &action)
                .ok_or_else(|| ActionError::Illegal {
                    action: action.clone(),
                })?;
        if !self.legal_actions(player).contains(&canonical) {
            return Err(ActionError::Illegal { action });
        }
        let log_start = self.log.len();
        self.execute(player, action);
        if self.winner.is_none() && self.complete_colors(self.current) >= SETS_TO_WIN {
            self.declare_winner(self.current);
        }
        Ok(self.log[log_start..].to_vec())
    }

    /// Replace a rent action's doubles with the equivalent lowest-id doubles. None if the doubles are invalid.
    fn canonical_doubles(&self, player: PlayerId, action: &Action) -> Option<Action> {
        let Action::PlayRent {
            card,
            set,
            target,
            doubles,
        } = action
        else {
            return Some(action.clone());
        };
        let hand = &self.player(player).hand;
        let mut seen = doubles.clone();
        seen.sort();
        seen.dedup();
        let valid = seen.len() == doubles.len()
            && doubles.iter().all(|d| {
                hand.contains(d) && card_def(*d).action() == Some(ActionKind::DoubleTheRent)
            });
        if !valid {
            return None;
        }
        let mut all: Vec<CardId> = hand
            .iter()
            .copied()
            .filter(|&c| card_def(c).action() == Some(ActionKind::DoubleTheRent))
            .collect();
        all.sort();
        Some(Action::PlayRent {
            card: *card,
            set: *set,
            target: *target,
            doubles: all[..doubles.len()].to_vec(),
        })
    }

    fn execute(&mut self, player: PlayerId, action: Action) {
        match action {
            Action::Bank { card } => {
                self.take_from_hand(player, card);
                self.player_mut(player).bank.push(card);
                self.plays_left -= 1;
                self.log.push(Event::Banked { player, card });
            }
            Action::PlayProperty { card, color, set } => {
                self.take_from_hand(player, card);
                self.plays_left -= 1;
                let set = self.put_in_set(player, card, color, set);
                self.log.push(Event::PropertyPlayed {
                    player,
                    card,
                    color,
                    set,
                });
            }
            Action::MoveProperty { card, color, set } => {
                self.remove_from_table(player, card);
                let set = self.put_in_set(player, card, color, set);
                self.log.push(Event::PropertyMoved {
                    player,
                    card,
                    color,
                    set,
                });
            }
            Action::PlayPassGo { card } => {
                self.play_card(player, card);
                self.log.push(Event::PassGo { player, card });
                self.draw(player, 2);
            }
            Action::PlayBirthday { card } => {
                let targets = self.seats_after(player);
                self.start_action(
                    player,
                    card,
                    Vec::new(),
                    Effect::Charge { amount: 2 },
                    targets,
                );
            }
            Action::PlayDebtCollector { card, target } => {
                self.start_action(
                    player,
                    card,
                    Vec::new(),
                    Effect::Charge { amount: 5 },
                    vec![target],
                );
            }
            Action::PlayRent {
                card,
                set,
                target,
                doubles,
            } => {
                let base = self
                    .player(player)
                    .sets
                    .iter()
                    .find(|s| s.id == set)
                    .expect("legal set")
                    .rent();
                let amount = base << doubles.len();
                let targets = match target {
                    Some(t) => vec![t],
                    None => self.seats_after(player),
                };
                self.start_action(player, card, doubles, Effect::Charge { amount }, targets);
            }
            Action::PlaySlyDeal { card, take } => {
                let target = self.owner_of(take);
                self.start_action(
                    player,
                    card,
                    Vec::new(),
                    Effect::SlyDeal { take },
                    vec![target],
                );
            }
            Action::PlayForcedDeal { card, give, take } => {
                let target = self.owner_of(take);
                self.start_action(
                    player,
                    card,
                    Vec::new(),
                    Effect::ForcedDeal { give, take },
                    vec![target],
                );
            }
            Action::PlayDealBreaker { card, set } => {
                let target = self.set_owner(set);
                self.start_action(
                    player,
                    card,
                    Vec::new(),
                    Effect::DealBreaker { set },
                    vec![target],
                );
            }
            Action::PlayHouse { card, set } | Action::PlayHotel { card, set } => {
                self.take_from_hand(player, card);
                self.plays_left -= 1;
                let is_house = card_def(card).action() == Some(ActionKind::House);
                let s = self
                    .player_mut(player)
                    .sets
                    .iter_mut()
                    .find(|s| s.id == set)
                    .expect("legal set");
                if is_house {
                    s.house = Some(card);
                } else {
                    s.hotel = Some(card);
                }
                self.log.push(Event::BuildingPlaced { player, card, set });
            }
            Action::EndTurn => {
                if self.player(player).hand.len() > HAND_LIMIT {
                    self.stage = Stage::Discard;
                } else {
                    self.end_turn();
                }
            }
            Action::Discard { card } => {
                self.take_from_hand(player, card);
                self.draw_pile.insert(0, card);
                self.log.push(Event::DiscardedToDrawPile { player });
                if self.player(player).hand.len() <= HAND_LIMIT {
                    self.end_turn();
                }
            }
            Action::JustSayNo { card } => {
                self.take_from_hand(player, card);
                self.play_pile.push(card);
                self.log.push(Event::JustSayNo { player, card });
                let Stage::Resolving(r) = &mut self.stage else {
                    unreachable!()
                };
                let Step::Respond {
                    responder,
                    just_say_no_count,
                } = &mut r.step
                else {
                    unreachable!()
                };
                *just_say_no_count += 1;
                *responder = if *responder == r.target {
                    r.actor
                } else {
                    r.target
                };
            }
            Action::Accept => {
                self.log.push(Event::Accepted { player });
                let Stage::Resolving(r) = &self.stage else {
                    unreachable!()
                };
                let Step::Respond {
                    just_say_no_count, ..
                } = r.step
                else {
                    unreachable!()
                };
                let (actor, target, effect) = (r.actor, r.target, r.effect.clone());
                if just_say_no_count % 2 == 1 {
                    self.log.push(Event::ActionCancelled { actor, target });
                    self.next_target();
                } else {
                    self.resolve(actor, target, effect);
                }
            }
            Action::PayCard { card } => {
                let Stage::Resolving(r) = &self.stage else {
                    unreachable!()
                };
                let (to, owed) = (
                    r.actor,
                    match r.step {
                        Step::Pay { owed, .. } => owed,
                        _ => unreachable!(),
                    },
                );
                if let Some(i) = self.player(player).bank.iter().position(|&c| c == card) {
                    self.player_mut(player).bank.remove(i);
                    self.player_mut(to).bank.push(card);
                } else {
                    let color = self.remove_from_table(player, card);
                    self.place_property(to, card, color);
                }
                self.log.push(Event::Paid {
                    from: player,
                    to,
                    card,
                });
                let Stage::Resolving(r) = &mut self.stage else {
                    unreachable!()
                };
                let Step::Pay { paid, .. } = &mut r.step else {
                    unreachable!()
                };
                *paid += card_def(card).value as u32;
                let paid = *paid;
                if paid >= owed || self.payable_cards(player).is_empty() {
                    self.log.push(Event::PaymentFinished {
                        from: player,
                        to,
                        owed,
                        paid,
                    });
                    self.next_target();
                }
            }
        }
    }

    /// Play an action card into the play pile and open the first target's response window.
    fn start_action(
        &mut self,
        actor: PlayerId,
        card: CardId,
        doubles: Vec<CardId>,
        effect: Effect,
        targets: Vec<PlayerId>,
    ) {
        self.play_card(actor, card);
        for &d in &doubles {
            self.play_card(actor, d);
        }
        self.log.push(Event::ActionPlayed {
            player: actor,
            card,
            doubles,
            effect: effect.clone(),
            targets: targets.clone(),
        });
        let mut queue = targets;
        let target = queue.remove(0);
        self.stage = Stage::Resolving(Resolution {
            actor,
            card,
            effect,
            target,
            queue,
            step: Step::Respond {
                responder: target,
                just_say_no_count: 0,
            },
        });
    }

    fn resolve(&mut self, actor: PlayerId, target: PlayerId, effect: Effect) {
        match effect {
            Effect::Charge { amount } => {
                if self.payable_cards(target).is_empty() {
                    self.log.push(Event::PaymentFinished {
                        from: target,
                        to: actor,
                        owed: amount,
                        paid: 0,
                    });
                    self.next_target();
                } else {
                    let Stage::Resolving(r) = &mut self.stage else {
                        unreachable!()
                    };
                    r.step = Step::Pay {
                        owed: amount,
                        paid: 0,
                    };
                }
            }
            Effect::SlyDeal { take } => {
                self.transfer_property(target, actor, take);
                self.next_target();
            }
            Effect::ForcedDeal { give, take } => {
                self.transfer_property(target, actor, take);
                self.transfer_property(actor, target, give);
                self.next_target();
            }
            Effect::DealBreaker { set } => {
                let sets = &mut self.player_mut(target).sets;
                let i = sets
                    .iter()
                    .position(|s| s.id == set)
                    .expect("set still on table");
                let set = sets.remove(i);
                let id = set.id;
                self.player_mut(actor).sets.push(set);
                self.log.push(Event::SetTaken {
                    from: target,
                    to: actor,
                    set: id,
                });
                self.next_target();
            }
        }
    }

    fn next_target(&mut self) {
        let Stage::Resolving(r) = &mut self.stage else {
            unreachable!()
        };
        if r.queue.is_empty() {
            self.stage = Stage::Play;
        } else {
            r.target = r.queue.remove(0);
            r.step = Step::Respond {
                responder: r.target,
                just_say_no_count: 0,
            };
        }
    }

    fn transfer_property(&mut self, from: PlayerId, to: PlayerId, card: CardId) {
        let color = self.remove_from_table(from, card);
        self.place_property(to, card, color);
        self.log.push(Event::PropertyTaken { from, to, card });
    }

    fn end_turn(&mut self) {
        self.log.push(Event::TurnEnded {
            player: self.current,
        });
        let next = PlayerId(((self.current.0 as usize + 1) % self.players.len()) as u8);
        self.start_turn(next);
    }

    fn start_turn(&mut self, player: PlayerId) {
        self.current = player;
        self.turn += 1;
        self.plays_left = PLAYS_PER_TURN;
        self.stage = Stage::Play;
        self.log.push(Event::TurnStarted {
            player,
            turn: self.turn,
        });
        if self.complete_colors(player) >= SETS_TO_WIN {
            self.declare_winner(player);
            return;
        }
        let count = if self.player(player).hand.is_empty() {
            5
        } else {
            2
        };
        self.draw(player, count);
    }

    fn declare_winner(&mut self, player: PlayerId) {
        self.winner = Some(player);
        self.stage = Stage::GameOver;
        self.log.push(Event::GameWon { winner: player });
    }

    /// Draw up to `count` cards. An empty draw pile is refilled by shuffling the play pile.
    fn draw(&mut self, player: PlayerId, count: usize) {
        let mut drawn = 0;
        for _ in 0..count {
            if self.draw_pile.is_empty() {
                if self.play_pile.is_empty() {
                    break;
                }
                let mut pile = std::mem::take(&mut self.play_pile);
                self.shuffle(&mut pile);
                self.log
                    .push(Event::DrawPileReshuffled { count: pile.len() });
                self.draw_pile = pile;
            }
            let card = self.draw_pile.pop().expect("refilled above");
            self.player_mut(player).hand.push(card);
            drawn += 1;
        }
        self.log.push(Event::Drew {
            player,
            count: drawn,
        });
    }

    fn play_card(&mut self, player: PlayerId, card: CardId) {
        self.take_from_hand(player, card);
        self.play_pile.push(card);
        self.plays_left -= 1;
    }

    fn take_from_hand(&mut self, player: PlayerId, card: CardId) {
        let hand = &mut self.player_mut(player).hand;
        let i = hand.iter().position(|&c| c == card).expect("card in hand");
        hand.remove(i);
    }

    /// Where `player` may put a card of `color`: each incomplete set of that color, or a new set.
    fn placements(&self, player: PlayerId, color: Color) -> Vec<Option<SetId>> {
        let mut out: Vec<Option<SetId>> = self
            .player(player)
            .sets
            .iter()
            .filter(|s| s.color == color && !s.is_complete())
            .map(|s| Some(s.id))
            .collect();
        out.push(None);
        out
    }

    /// Received cards (payment, steals) join the first incomplete set of their color, or a new set.
    fn place_property(&mut self, player: PlayerId, card: CardId, color: Color) -> SetId {
        let target = self
            .player(player)
            .sets
            .iter()
            .find(|s| s.color == color && !s.is_complete())
            .map(|s| s.id);
        self.put_in_set(player, card, color, target)
    }

    fn put_in_set(
        &mut self,
        player: PlayerId,
        card: CardId,
        color: Color,
        set: Option<SetId>,
    ) -> SetId {
        let next_id = SetId(self.next_set_id);
        let sets = &mut self.player_mut(player).sets;
        if let Some(id) = set {
            sets.iter_mut()
                .find(|s| s.id == id)
                .expect("legal set")
                .cards
                .push(card);
            return id;
        }
        sets.push(PropertySet {
            id: next_id,
            color,
            cards: vec![card],
            house: None,
            hotel: None,
        });
        self.next_set_id += 1;
        next_id
    }

    /// Remove a property card from its set and return the set color. A set left with no cards
    /// and no buildings disappears; buildings stay on their set.
    fn remove_from_table(&mut self, player: PlayerId, card: CardId) -> Color {
        let sets = &mut self.player_mut(player).sets;
        let si = sets
            .iter()
            .position(|s| s.cards.contains(&card))
            .expect("card on table");
        let set = &mut sets[si];
        set.cards.retain(|&c| c != card);
        let color = set.color;
        if set.cards.is_empty() && set.house.is_none() && set.hotel.is_none() {
            sets.remove(si);
        }
        color
    }

    fn payable_cards(&self, player: PlayerId) -> Vec<CardId> {
        let p = self.player(player);
        let mut cards = p.bank.clone();
        for set in &p.sets {
            cards.extend(
                set.cards
                    .iter()
                    .copied()
                    .filter(|&c| !card_def(c).is_multicolor_wild()),
            );
        }
        cards
    }

    /// Property cards in sets that are not full.
    fn unprotected_properties(&self, player: PlayerId) -> Vec<CardId> {
        self.player(player)
            .sets
            .iter()
            .filter(|s| !s.is_complete())
            .flat_map(|s| s.cards.iter().copied())
            .collect()
    }

    pub(crate) fn complete_colors(&self, player: PlayerId) -> usize {
        let mut colors: Vec<Color> = self
            .player(player)
            .sets
            .iter()
            .filter(|s| s.is_complete())
            .map(|s| s.color)
            .collect();
        colors.sort();
        colors.dedup();
        colors.len()
    }

    fn owner_of(&self, card: CardId) -> PlayerId {
        let i = self
            .players
            .iter()
            .position(|p| p.sets.iter().any(|s| s.cards.contains(&card)))
            .expect("card on table");
        PlayerId(i as u8)
    }

    fn set_owner(&self, set: SetId) -> PlayerId {
        let i = self
            .players
            .iter()
            .position(|p| p.sets.iter().any(|s| s.id == set))
            .expect("set on table");
        PlayerId(i as u8)
    }

    /// Other players in seat order, starting after `player`.
    fn seats_after(&self, player: PlayerId) -> Vec<PlayerId> {
        let n = self.players.len();
        (1..n)
            .map(|k| PlayerId(((player.0 as usize + k) % n) as u8))
            .collect()
    }

    fn player(&self, id: PlayerId) -> &PlayerState {
        &self.players[id.0 as usize]
    }

    fn player_mut(&mut self, id: PlayerId) -> &mut PlayerState {
        &mut self.players[id.0 as usize]
    }

    /// SplitMix64.
    fn next_u64(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn shuffle(&mut self, cards: &mut [CardId]) {
        for i in (1..cards.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            cards.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests;
