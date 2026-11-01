use super::*;
use crate::cards::{CardDef, PACK};

// ---------- helpers ----------

/// A game with every card in the draw pile, player 0 to play with 3 plays left.
fn setup(n: usize) -> Game {
    let mut g = Game::new(GameConfig::new(n, 7)).unwrap();
    let mut all: Vec<CardId> = g.card_ids().collect();
    all.reverse();
    g.draw_pile = all;
    g.play_pile.clear();
    for p in &mut g.players {
        *p = PlayerState {
            hand: Vec::new(),
            bank: Vec::new(),
            sets: Vec::new(),
        };
    }
    g.current = PlayerId(0);
    g.turn = 1;
    g.plays_left = 3;
    g.stage = Stage::Play;
    g.winner = None;
    g.log.clear();
    g
}

fn pull(g: &mut Game, pred: impl Fn(&CardDef) -> bool) -> CardId {
    let i = g
        .draw_pile
        .iter()
        .position(|&c| pred(card_def(c)))
        .expect("matching card left in draw pile");
    g.draw_pile.remove(i)
}

fn named(g: &mut Game, name: &str) -> CardId {
    pull(g, |d| d.name == name)
}

fn action_card(g: &mut Game, kind: ActionKind) -> CardId {
    pull(g, |d| d.action() == Some(kind))
}

fn money(g: &mut Game, value: u8) -> CardId {
    pull(g, |d| d.kind == CardKind::Money && d.value == value)
}

fn rent_card(g: &mut Game, any: bool) -> CardId {
    pull(
        g,
        |d| matches!(d.kind, CardKind::Rent { colors } if (colors.len() == 10) == any),
    )
}

fn multicolor_wild(g: &mut Game) -> CardId {
    pull(g, |d| d.is_multicolor_wild())
}

fn to_hand(g: &mut Game, p: u8, card: CardId) -> CardId {
    g.players[p as usize].hand.push(card);
    card
}

fn to_bank(g: &mut Game, p: u8, card: CardId) {
    g.players[p as usize].bank.push(card);
}

/// Put named properties on the table as one set.
fn to_set(g: &mut Game, p: u8, color: Color, names: &[&str]) -> SetId {
    let cards: Vec<CardId> = names.iter().map(|n| named(g, n)).collect();
    to_set_cards(g, p, color, cards)
}

fn to_set_cards(g: &mut Game, p: u8, color: Color, cards: Vec<CardId>) -> SetId {
    let id = SetId(g.next_set_id);
    g.next_set_id += 1;
    g.players[p as usize].sets.push(PropertySet {
        id,
        color,
        cards,
        house: None,
        hotel: None,
    });
    id
}

fn full_dark_blue(g: &mut Game, p: u8) -> SetId {
    to_set(g, p, Color::DarkBlue, &["Park Place", "Boardwalk"])
}
fn full_brown(g: &mut Game, p: u8) -> SetId {
    to_set(
        g,
        p,
        Color::Brown,
        &["Mediterranean Avenue", "Baltic Avenue"],
    )
}
fn full_utility(g: &mut Game, p: u8) -> SetId {
    to_set(g, p, Color::Utility, &["Electric Company", "Water Works"])
}

fn act(g: &mut Game, p: u8, a: Action) -> Vec<Event> {
    match g.apply(PlayerId(p), a.clone()) {
        Ok(events) => events,
        Err(e) => panic!(
            "{a:?} by {p} failed: {e}; legal: {:?}",
            g.legal_actions(PlayerId(p))
        ),
    }
}

fn all_cards(g: &Game) -> Vec<CardId> {
    let mut v: Vec<CardId> = g.draw_pile.iter().chain(&g.play_pile).copied().collect();
    for p in &g.players {
        v.extend(&p.hand);
        v.extend(&p.bank);
        for s in &p.sets {
            v.extend(&s.cards);
            v.extend(s.house);
            v.extend(s.hotel);
        }
    }
    v.sort();
    v
}

fn assert_conserved(g: &Game) {
    let expected: Vec<CardId> = g.card_ids().collect();
    assert_eq!(all_cards(g), expected, "every card is in exactly one zone");
}

fn bank_total(g: &Game, p: u8) -> u32 {
    g.players[p as usize]
        .bank
        .iter()
        .map(|&c| card_def(c).value as u32)
        .sum()
}

// ---------- deck ----------

#[test]
fn pack_matches_2008_contents() {
    let count = |f: &dyn Fn(&CardDef) -> bool| PACK.iter().filter(|d| f(d)).count();
    assert_eq!(PACK.len(), 106);
    assert_eq!(count(&|d| matches!(d.kind, CardKind::Property { .. })), 28);
    assert_eq!(count(&|d| matches!(d.kind, CardKind::Wild { .. })), 11);
    assert_eq!(count(&|d| matches!(d.kind, CardKind::Action { .. })), 34);
    assert_eq!(count(&|d| matches!(d.kind, CardKind::Rent { .. })), 13);
    assert_eq!(count(&|d| d.kind == CardKind::Money), 20);
    let money: u32 = PACK
        .iter()
        .filter(|d| d.kind == CardKind::Money)
        .map(|d| d.value as u32)
        .sum();
    assert_eq!(money, 57);
    for color in crate::cards::ALL_COLORS {
        let n = count(&|d| d.kind == CardKind::Property { color });
        assert_eq!(n, color.set_size(), "{color:?}");
    }
    assert_eq!(count(&|d| d.action() == Some(ActionKind::PassGo)), 10);
    assert_eq!(count(&|d| d.is_multicolor_wild()), 2);
    assert!(PACK.iter().all(|d| !d.is_property() || !d.bankable()));
}

#[test]
fn card_ids_are_stable_and_packs_scale() {
    let g = Game::new(GameConfig::new(5, 1)).unwrap();
    assert_eq!(g.card_ids().count(), 106);
    let g = Game::new(GameConfig::new(6, 1)).unwrap();
    assert_eq!(g.card_ids().count(), 212);
    assert_eq!(card_def(CardId(106 + 19)).name, "10M");
    assert_eq!(card_def(CardId(19)), card_def(CardId(125)));
}

#[test]
fn player_count_limits() {
    assert!(Game::new(GameConfig::new(1, 0)).is_err());
    assert!(Game::new(GameConfig::new(43, 0)).is_err());
    assert_eq!(MAX_PLAYERS, 42);
    let g = Game::new(GameConfig::new(42, 0)).unwrap();
    assert_conserved(&g);
    assert_eq!(g.draw_pile.len(), 0);
}

// ---------- setup and determinism ----------

#[test]
fn new_game_deals_and_starts_first_turn() {
    let g = Game::new(GameConfig::new(4, 99)).unwrap();
    assert_eq!(g.players[0].hand.len(), 7);
    assert!(g.players[1..].iter().all(|p| p.hand.len() == 5));
    assert_eq!(
        g.phase(),
        Phase::Play {
            player: PlayerId(0),
            plays_left: 3
        }
    );
    assert_eq!(g.pending_actors(), vec![PlayerId(0)]);
    assert_conserved(&g);
}

#[test]
fn same_seed_same_game() {
    let a = Game::new(GameConfig::new(3, 42)).unwrap();
    let b = Game::new(GameConfig::new(3, 42)).unwrap();
    let c = Game::new(GameConfig::new(3, 43)).unwrap();
    assert_eq!(a, b);
    assert_ne!(a.players[0].hand, c.players[0].hand);
}

#[test]
fn game_round_trips_through_json() {
    let mut g = Game::new(GameConfig::new(3, 5)).unwrap();
    act(&mut g, 0, Action::EndTurn);
    let json = serde_json::to_string(&g).unwrap();
    let back: Game = serde_json::from_str(&json).unwrap();
    assert_eq!(back, g);
}

// ---------- hidden information ----------

#[test]
fn observations_hide_other_hands_and_deck() {
    let g = Game::new(GameConfig::new(3, 11)).unwrap();
    let spectator = g.observation(None);
    assert!(spectator.hand.is_empty());
    assert!(spectator.legal_actions.is_empty());
    let p1 = g.observation(Some(PlayerId(1)));
    assert_eq!(p1.hand, g.players[1].hand);
    assert!(p1.legal_actions.is_empty(), "not player 1's decision");
    assert_eq!(p1.players[0].hand_count, 7);
    let json = serde_json::to_value(&p1).unwrap();
    let keys: Vec<String> = json.as_object().unwrap().keys().cloned().collect();
    assert!(!keys
        .iter()
        .any(|k| k.contains("draw_pile") || k.contains("rng")));
    let p0 = g.observation(Some(PlayerId(0)));
    assert_eq!(p0.legal_actions, g.legal_actions(PlayerId(0)));
}

#[test]
fn discard_to_bottom_of_draw_pile_is_not_revealed() {
    let mut g = setup(2);
    while g.players[0].hand.len() < 9 {
        let c = action_card(&mut g, ActionKind::PassGo);
        to_hand(&mut g, 0, c);
    }
    act(&mut g, 0, Action::EndTurn);
    assert_eq!(
        g.phase(),
        Phase::Discard {
            player: PlayerId(0),
            excess: 2
        }
    );
    let card = g.players[0].hand[0];
    let events = act(&mut g, 0, Action::Discard { card });
    assert_eq!(
        events,
        vec![Event::DiscardedToDrawPile {
            player: PlayerId(0)
        }]
    );
    assert_eq!(
        g.draw_pile[0], card,
        "excess card goes to the bottom of the draw pile"
    );
    let card = g.players[0].hand[0];
    act(&mut g, 0, Action::Discard { card });
    assert_eq!(g.players[0].hand.len(), 7);
    assert_eq!(g.current, PlayerId(1));
    assert_conserved(&g);
}

// ---------- errors ----------

#[test]
fn rejected_actions_leave_state_unchanged() {
    let mut g = Game::new(GameConfig::new(3, 3)).unwrap();
    let before = g.clone();
    let property_or_money = g.players[0].hand[0];
    let not_in_hand = g.players[1].hand[0];
    assert!(matches!(
        g.apply(PlayerId(1), Action::EndTurn),
        Err(ActionError::NotYourDecision { .. })
    ));
    assert!(matches!(
        g.apply(PlayerId(9), Action::EndTurn),
        Err(ActionError::UnknownPlayer { .. })
    ));
    assert!(g
        .apply(PlayerId(0), Action::Bank { card: not_in_hand })
        .is_err());
    assert!(g.apply(PlayerId(0), Action::Accept).is_err());
    assert!(g
        .apply(
            PlayerId(0),
            Action::PayCard {
                card: property_or_money
            }
        )
        .is_err());
    assert!(g
        .apply(
            PlayerId(0),
            Action::PlayRent {
                card: property_or_money,
                set: SetId(99),
                target: None,
                doubles: vec![CardId(5000)]
            }
        )
        .is_err());
    assert_eq!(g, before);
}

#[test]
fn properties_cannot_be_banked() {
    let mut g = setup(2);
    let wild = named(&mut g, "Property Wild Card: Pink/Orange");
    to_hand(&mut g, 0, wild);
    let prop = named(&mut g, "Boardwalk");
    to_hand(&mut g, 0, prop);
    assert!(g.apply(PlayerId(0), Action::Bank { card: wild }).is_err());
    assert!(g.apply(PlayerId(0), Action::Bank { card: prop }).is_err());
}

// ---------- turn flow ----------

#[test]
fn three_plays_then_only_end_turn_or_moves() {
    let mut g = setup(2);
    for _ in 0..4 {
        let c = money(&mut g, 1);
        to_hand(&mut g, 0, c);
    }
    for _ in 0..3 {
        let c = g.players[0].hand[0];
        act(&mut g, 0, Action::Bank { card: c });
    }
    assert_eq!(g.legal_actions(PlayerId(0)), vec![Action::EndTurn]);
}

#[test]
fn empty_hand_draws_five_otherwise_two() {
    let mut g = setup(2);
    act(&mut g, 0, Action::EndTurn);
    assert_eq!(g.players[1].hand.len(), 5);
    act(&mut g, 1, Action::EndTurn);
    assert_eq!(g.players[0].hand.len(), 5);
    act(&mut g, 0, Action::EndTurn);
    assert_eq!(g.players[1].hand.len(), 7);
}

#[test]
fn empty_draw_pile_reshuffles_play_pile() {
    let mut g = setup(2);
    let pass = action_card(&mut g, ActionKind::PassGo);
    to_hand(&mut g, 0, pass);
    let rest: Vec<CardId> = g.draw_pile.drain(..).collect();
    g.play_pile = rest;
    act(&mut g, 0, Action::PlayPassGo { card: pass });
    assert_eq!(g.players[0].hand.len(), 2);
    assert!(g
        .log
        .iter()
        .any(|e| matches!(e, Event::DrawPileReshuffled { .. })));
    assert_conserved(&g);
}

#[test]
fn pass_go_draws_two_and_uses_a_play() {
    let mut g = setup(2);
    let a = action_card(&mut g, ActionKind::PassGo);
    to_hand(&mut g, 0, a);
    let b = action_card(&mut g, ActionKind::PassGo);
    to_hand(&mut g, 0, b);
    act(&mut g, 0, Action::PlayPassGo { card: a });
    act(&mut g, 0, Action::PlayPassGo { card: b });
    assert_eq!(g.players[0].hand.len(), 4);
    assert_eq!(g.plays_left, 1);
}

// ---------- just say no ----------

#[test]
fn response_window_opens_without_just_say_no() {
    let mut g = setup(2);
    let debt = action_card(&mut g, ActionKind::DebtCollector);
    to_hand(&mut g, 0, debt);
    act(
        &mut g,
        0,
        Action::PlayDebtCollector {
            card: debt,
            target: PlayerId(1),
        },
    );
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            responder: PlayerId(1),
            ..
        }
    ));
    assert_eq!(g.legal_actions(PlayerId(1)), vec![Action::Accept]);
    act(&mut g, 1, Action::Accept);
    // Player 1 has nothing on the table, so nothing is paid.
    assert_eq!(
        g.phase(),
        Phase::Play {
            player: PlayerId(0),
            plays_left: 2
        }
    );
}

#[test]
fn just_say_no_chain() {
    let mut g = setup(2);
    let debt = action_card(&mut g, ActionKind::DebtCollector);
    to_hand(&mut g, 0, debt);
    let a1 = action_card(&mut g, ActionKind::JustSayNo);
    to_hand(&mut g, 0, a1);
    let t1 = action_card(&mut g, ActionKind::JustSayNo);
    to_hand(&mut g, 1, t1);
    let t2 = action_card(&mut g, ActionKind::JustSayNo);
    to_hand(&mut g, 1, t2);
    let five = money(&mut g, 5);
    to_bank(&mut g, 1, five);
    act(
        &mut g,
        0,
        Action::PlayDebtCollector {
            card: debt,
            target: PlayerId(1),
        },
    );
    act(&mut g, 1, Action::JustSayNo { card: t1 });
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            responder: PlayerId(0),
            just_say_no_count: 1,
            ..
        }
    ));
    act(&mut g, 0, Action::JustSayNo { card: a1 });
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            responder: PlayerId(1),
            just_say_no_count: 2,
            ..
        }
    ));
    act(&mut g, 1, Action::JustSayNo { card: t2 });
    act(&mut g, 0, Action::Accept);
    assert!(g
        .log
        .iter()
        .any(|e| matches!(e, Event::ActionCancelled { .. })));
    assert_eq!(bank_total(&g, 1), 5);
    assert_eq!(g.plays_left, 2, "responses do not use plays");
    assert_conserved(&g);
}

#[test]
fn countered_just_say_no_lets_the_action_resolve() {
    let mut g = setup(2);
    let debt = action_card(&mut g, ActionKind::DebtCollector);
    to_hand(&mut g, 0, debt);
    let a1 = action_card(&mut g, ActionKind::JustSayNo);
    to_hand(&mut g, 0, a1);
    let t1 = action_card(&mut g, ActionKind::JustSayNo);
    to_hand(&mut g, 1, t1);
    let five = money(&mut g, 5);
    to_bank(&mut g, 1, five);
    act(
        &mut g,
        0,
        Action::PlayDebtCollector {
            card: debt,
            target: PlayerId(1),
        },
    );
    act(&mut g, 1, Action::JustSayNo { card: t1 });
    act(&mut g, 0, Action::JustSayNo { card: a1 });
    act(&mut g, 1, Action::Accept);
    act(&mut g, 1, Action::PayCard { card: five });
    assert_eq!(bank_total(&g, 0), 5);
    assert_eq!(
        g.phase(),
        Phase::Play {
            player: PlayerId(0),
            plays_left: 2
        }
    );
}

#[test]
fn birthday_targets_resolve_serially_and_independently() {
    let mut g = setup(4);
    let bday = action_card(&mut g, ActionKind::ItsMyBirthday);
    to_hand(&mut g, 0, bday);
    let jsn = action_card(&mut g, ActionKind::JustSayNo);
    to_hand(&mut g, 1, jsn);
    for p in 1..4 {
        let c = money(&mut g, 2);
        to_bank(&mut g, p, c);
    }
    act(&mut g, 0, Action::PlayBirthday { card: bday });
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            target: PlayerId(1),
            ..
        }
    ));
    act(&mut g, 1, Action::JustSayNo { card: jsn });
    act(&mut g, 0, Action::Accept);
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            target: PlayerId(2),
            responder: PlayerId(2),
            ..
        }
    ));
    act(&mut g, 2, Action::Accept);
    let c = g.players[2].bank[0];
    act(&mut g, 2, Action::PayCard { card: c });
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            target: PlayerId(3),
            ..
        }
    ));
    act(&mut g, 3, Action::Accept);
    let c = g.players[3].bank[0];
    act(&mut g, 3, Action::PayCard { card: c });
    assert_eq!(bank_total(&g, 0), 4);
    assert_eq!(bank_total(&g, 1), 2);
    assert!(matches!(g.phase(), Phase::Play { .. }));
}

// ---------- payment ----------

#[test]
fn payment_gives_no_change_and_payer_chooses() {
    let mut g = setup(2);
    let bday = action_card(&mut g, ActionKind::ItsMyBirthday);
    to_hand(&mut g, 0, bday);
    let three = money(&mut g, 3);
    let one = money(&mut g, 1);
    to_bank(&mut g, 1, three);
    to_bank(&mut g, 1, one);
    act(&mut g, 0, Action::PlayBirthday { card: bday });
    act(&mut g, 1, Action::Accept);
    let legal = g.legal_actions(PlayerId(1));
    assert!(
        legal.contains(&Action::PayCard { card: three })
            && legal.contains(&Action::PayCard { card: one })
    );
    act(&mut g, 1, Action::PayCard { card: three });
    assert_eq!(bank_total(&g, 0), 3);
    assert_eq!(bank_total(&g, 1), 1);
    assert!(matches!(g.phase(), Phase::Play { .. }));
}

#[test]
fn paid_properties_go_to_receivers_collection_and_multicolor_wild_is_not_payable() {
    let mut g = setup(2);
    let debt = action_card(&mut g, ActionKind::DebtCollector);
    to_hand(&mut g, 0, debt);
    to_set(&mut g, 1, Color::Pink, &["States Avenue"]);
    let wild = multicolor_wild(&mut g);
    to_set_cards(&mut g, 1, Color::Red, vec![wild]);
    let one = money(&mut g, 1);
    to_bank(&mut g, 1, one);
    act(
        &mut g,
        0,
        Action::PlayDebtCollector {
            card: debt,
            target: PlayerId(1),
        },
    );
    act(&mut g, 1, Action::Accept);
    assert!(!g
        .legal_actions(PlayerId(1))
        .contains(&Action::PayCard { card: wild }));
    let states = g.players[1].sets[0].cards[0];
    act(&mut g, 1, Action::PayCard { card: states });
    assert_eq!(g.players[0].sets[0].color, Color::Pink);
    assert_eq!(g.players[0].sets[0].cards, vec![states]);
    // Owes 5, paid 2: must keep paying while payable cards remain.
    act(&mut g, 1, Action::PayCard { card: one });
    assert!(
        matches!(g.phase(), Phase::Play { .. }),
        "payer has only the unpayable wild left"
    );
    assert_eq!(g.players[1].sets.len(), 1);
    assert!(g.log.iter().any(|e| matches!(
        e,
        Event::PaymentFinished {
            owed: 5,
            paid: 3,
            ..
        }
    )));
}

// ---------- rent ----------

#[test]
fn rent_ladder_buildings_and_doubles() {
    let mut g = setup(2);
    let set = full_dark_blue(&mut g, 0);
    let any = rent_card(&mut g, true);
    to_hand(&mut g, 0, any);
    let d1 = action_card(&mut g, ActionKind::DoubleTheRent);
    to_hand(&mut g, 0, d1);
    let d2 = action_card(&mut g, ActionKind::DoubleTheRent);
    to_hand(&mut g, 0, d2);
    {
        let s = &mut g.players[0].sets[0];
        s.house = Some(pull_house(&mut g.draw_pile));
    }
    assert_eq!(g.players[0].sets[0].rent(), 8 + 3);
    for _ in 0..20 {
        let c = money(&mut g, 1);
        to_bank(&mut g, 1, c);
        if g.players[1].bank.len() == 6 {
            break;
        }
    }
    let legal = g.legal_actions(PlayerId(0));
    let mut ds = vec![d1, d2];
    ds.sort();
    assert!(legal.contains(&Action::PlayRent {
        card: any,
        set,
        target: Some(PlayerId(1)),
        doubles: ds.clone()
    }));
    // Pass the doubles in the other order: still accepted.
    act(
        &mut g,
        0,
        Action::PlayRent {
            card: any,
            set,
            target: Some(PlayerId(1)),
            doubles: vec![ds[1], ds[0]],
        },
    );
    assert_eq!(g.plays_left, 0);
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            effect: Effect::Charge { amount: 44 },
            ..
        }
    ));
    act(&mut g, 1, Action::Accept);
    while matches!(g.phase(), Phase::Pay { .. }) {
        let c = g.players[1].bank[0];
        act(&mut g, 1, Action::PayCard { card: c });
    }
    assert_eq!(bank_total(&g, 0), 6, "payer gives everything when short");
    assert_conserved(&g);
}

fn pull_house(pile: &mut Vec<CardId>) -> CardId {
    let i = pile
        .iter()
        .position(|&c| card_def(c).action() == Some(ActionKind::House))
        .unwrap();
    pile.remove(i)
}

#[test]
fn two_color_rent_charges_everyone_any_rent_one_player() {
    let mut g = setup(3);
    let rr = to_set(
        &mut g,
        0,
        Color::Railroad,
        &["Reading Railroad", "Short Line"],
    );
    let rent = pull(&mut g, |d| {
        d.kind
            == CardKind::Rent {
                colors: &[Color::Railroad, Color::Utility],
            }
    });
    to_hand(&mut g, 0, rent);
    let legal = g.legal_actions(PlayerId(0));
    assert!(legal.contains(&Action::PlayRent {
        card: rent,
        set: rr,
        target: None,
        doubles: vec![]
    }));
    assert!(!legal.iter().any(|a| matches!(
        a,
        Action::PlayRent {
            target: Some(_),
            ..
        }
    )));
    act(
        &mut g,
        0,
        Action::PlayRent {
            card: rent,
            set: rr,
            target: None,
            doubles: vec![],
        },
    );
    assert!(matches!(
        g.phase(),
        Phase::Respond {
            effect: Effect::Charge { amount: 2 },
            target: PlayerId(1),
            ..
        }
    ));
    let any = rent_card(&mut g, true);
    to_hand(&mut g, 0, any);
    act(&mut g, 1, Action::Accept);
    act(&mut g, 2, Action::Accept);
    let legal = g.legal_actions(PlayerId(0));
    assert!(legal.contains(&Action::PlayRent {
        card: any,
        set: rr,
        target: Some(PlayerId(2)),
        doubles: vec![]
    }));
    assert!(!legal.contains(&Action::PlayRent {
        card: any,
        set: rr,
        target: None,
        doubles: vec![]
    }));
}

#[test]
fn rent_needs_a_matching_set() {
    let mut g = setup(2);
    let rent = rent_card(&mut g, true);
    to_hand(&mut g, 0, rent);
    assert!(!g
        .legal_actions(PlayerId(0))
        .iter()
        .any(|a| matches!(a, Action::PlayRent { card, .. } if *card == rent)));
}

// ---------- property actions ----------

#[test]
fn sly_deal_and_forced_deal_skip_full_sets() {
    let mut g = setup(2);
    full_dark_blue(&mut g, 1);
    to_set(&mut g, 1, Color::Green, &["Pacific Avenue"]);
    to_set(&mut g, 0, Color::Red, &["Kentucky Avenue"]);
    full_brown(&mut g, 0);
    let sly = action_card(&mut g, ActionKind::SlyDeal);
    to_hand(&mut g, 0, sly);
    let forced = action_card(&mut g, ActionKind::ForcedDeal);
    to_hand(&mut g, 0, forced);
    let legal = g.legal_actions(PlayerId(0));
    let pacific = g.players[1].sets[1].cards[0];
    let kentucky = g.players[0].sets[0].cards[0];
    let sly_takes: Vec<_> = legal
        .iter()
        .filter(|a| matches!(a, Action::PlaySlyDeal { .. }))
        .collect();
    assert_eq!(
        sly_takes,
        vec![&Action::PlaySlyDeal {
            card: sly,
            take: pacific
        }]
    );
    let forced_deals: Vec<_> = legal
        .iter()
        .filter(|a| matches!(a, Action::PlayForcedDeal { .. }))
        .collect();
    assert_eq!(
        forced_deals,
        vec![&Action::PlayForcedDeal {
            card: forced,
            give: kentucky,
            take: pacific
        }]
    );
    act(
        &mut g,
        0,
        Action::PlayForcedDeal {
            card: forced,
            give: kentucky,
            take: pacific,
        },
    );
    act(&mut g, 1, Action::Accept);
    assert!(g.players[0]
        .sets
        .iter()
        .any(|s| s.cards == vec![pacific] && s.color == Color::Green));
    assert!(g.players[1]
        .sets
        .iter()
        .any(|s| s.cards == vec![kentucky] && s.color == Color::Red));
    assert_conserved(&g);
}

#[test]
fn deal_breaker_takes_set_with_buildings_and_keeps_id() {
    let mut g = setup(2);
    let set = full_dark_blue(&mut g, 1);
    let house = pull_house(&mut g.draw_pile);
    g.players[1].sets[0].house = Some(house);
    let db = action_card(&mut g, ActionKind::DealBreaker);
    to_hand(&mut g, 0, db);
    act(&mut g, 0, Action::PlayDealBreaker { card: db, set });
    act(&mut g, 1, Action::Accept);
    assert!(g.players[1].sets.is_empty());
    assert_eq!(g.players[0].sets[0].id, set);
    assert_eq!(g.players[0].sets[0].house, Some(house));
    assert_conserved(&g);
}

#[test]
fn house_then_hotel_not_on_railroad_or_utility() {
    let mut g = setup(2);
    let blue = full_dark_blue(&mut g, 0);
    full_utility(&mut g, 0);
    let house = action_card(&mut g, ActionKind::House);
    to_hand(&mut g, 0, house);
    let hotel = action_card(&mut g, ActionKind::Hotel);
    to_hand(&mut g, 0, hotel);
    let legal = g.legal_actions(PlayerId(0));
    assert!(legal.contains(&Action::PlayHouse {
        card: house,
        set: blue
    }));
    assert_eq!(
        legal
            .iter()
            .filter(|a| matches!(a, Action::PlayHouse { .. }))
            .count(),
        1
    );
    assert!(!legal.iter().any(|a| matches!(a, Action::PlayHotel { .. })));
    act(
        &mut g,
        0,
        Action::PlayHouse {
            card: house,
            set: blue,
        },
    );
    act(
        &mut g,
        0,
        Action::PlayHotel {
            card: hotel,
            set: blue,
        },
    );
    assert_eq!(g.players[0].sets[0].rent(), 8 + 3 + 4);
}

#[test]
fn wild_can_leave_a_full_set_and_buildings_stay() {
    let mut g = setup(2);
    let wild = named(&mut g, "Property Wild Card: Dark Blue/Green");
    let park = named(&mut g, "Park Place");
    let blue = to_set_cards(&mut g, 0, Color::DarkBlue, vec![park, wild]);
    let house = pull_house(&mut g.draw_pile);
    g.players[0].sets[0].house = Some(house);
    act(
        &mut g,
        0,
        Action::MoveProperty {
            card: wild,
            color: Color::Green,
            set: None,
        },
    );
    assert_eq!(g.plays_left, 3, "moving a wild is free");
    let s = g.players[0].sets.iter().find(|s| s.id == blue).unwrap();
    assert_eq!(s.house, Some(house));
    assert!(!s.is_complete());
    assert_eq!(
        s.rent(),
        3,
        "building bonus counts only while the set is full"
    );
    act(
        &mut g,
        0,
        Action::MoveProperty {
            card: wild,
            color: Color::DarkBlue,
            set: Some(blue),
        },
    );
    let s = g.players[0].sets.iter().find(|s| s.id == blue).unwrap();
    assert_eq!(s.rent(), 11);
    assert_conserved(&g);
}

#[test]
fn duplicate_color_sets_count_once_and_are_separate() {
    let mut g = setup(6);
    let a = full_dark_blue(&mut g, 0);
    let b = full_dark_blue(&mut g, 0);
    full_brown(&mut g, 0);
    assert_ne!(a, b);
    assert_eq!(g.complete_colors(PlayerId(0)), 2);
    let house = action_card(&mut g, ActionKind::House);
    to_hand(&mut g, 0, house);
    let legal = g.legal_actions(PlayerId(0));
    assert!(legal.contains(&Action::PlayHouse {
        card: house,
        set: a
    }));
    assert!(legal.contains(&Action::PlayHouse {
        card: house,
        set: b
    }));
    act(
        &mut g,
        0,
        Action::PlayHouse {
            card: house,
            set: b,
        },
    );
    assert_eq!(g.players[0].sets[0].house, None);
    assert!(g.winner.is_none());
}

#[test]
fn players_choose_set_destinations_and_can_merge_split_sets() {
    let mut g = setup(2);
    let a = named(&mut g, "Kentucky Avenue");
    let b = named(&mut g, "Indiana Avenue");
    to_hand(&mut g, 0, a);
    to_hand(&mut g, 0, b);
    act(
        &mut g,
        0,
        Action::PlayProperty {
            card: a,
            color: Color::Red,
            set: None,
        },
    );
    let first = g.players[0].sets[0].id;
    assert!(g
        .legal_actions(PlayerId(0))
        .contains(&Action::PlayProperty {
            card: b,
            color: Color::Red,
            set: Some(first)
        }));
    act(
        &mut g,
        0,
        Action::PlayProperty {
            card: b,
            color: Color::Red,
            set: None,
        },
    );
    assert_eq!(g.players[0].sets.len(), 2);
    act(
        &mut g,
        0,
        Action::MoveProperty {
            card: b,
            color: Color::Red,
            set: Some(first),
        },
    );
    assert_eq!(g.players[0].sets.len(), 1);
    assert_eq!(g.players[0].sets[0].cards, vec![a, b]);
    assert_eq!(g.players[0].sets[0].rent(), 3);
}

#[test]
fn set_with_buildings_but_no_cards_can_be_refilled() {
    let mut g = setup(2);
    let blue = full_dark_blue(&mut g, 0);
    let house = pull_house(&mut g.draw_pile);
    g.players[0].sets[0].house = Some(house);
    let cards = std::mem::take(&mut g.players[0].sets[0].cards);
    g.draw_pile.extend(cards);
    let park = named(&mut g, "Park Place");
    let boardwalk = named(&mut g, "Boardwalk");
    to_hand(&mut g, 0, park);
    to_hand(&mut g, 0, boardwalk);
    act(
        &mut g,
        0,
        Action::PlayProperty {
            card: park,
            color: Color::DarkBlue,
            set: Some(blue),
        },
    );
    act(
        &mut g,
        0,
        Action::PlayProperty {
            card: boardwalk,
            color: Color::DarkBlue,
            set: Some(blue),
        },
    );
    assert_eq!(g.players[0].sets[0].rent(), 11);
}

#[test]
fn multicolor_wilds_alone_can_form_a_set() {
    let mut g = setup(2);
    let w1 = multicolor_wild(&mut g);
    let w2 = multicolor_wild(&mut g);
    to_hand(&mut g, 0, w1);
    to_hand(&mut g, 0, w2);
    act(
        &mut g,
        0,
        Action::PlayProperty {
            card: w1,
            color: Color::Brown,
            set: None,
        },
    );
    let set = g.players[0].sets[0].id;
    act(
        &mut g,
        0,
        Action::PlayProperty {
            card: w2,
            color: Color::Brown,
            set: Some(set),
        },
    );
    assert!(g.players[0].sets[0].is_complete());
    assert_eq!(g.players[0].sets[0].rent(), 2);
}

// ---------- winning ----------

#[test]
fn win_on_own_turn_is_immediate() {
    let mut g = setup(2);
    full_dark_blue(&mut g, 0);
    full_brown(&mut g, 0);
    let util = to_set(&mut g, 0, Color::Utility, &["Electric Company"]);
    let water = named(&mut g, "Water Works");
    to_hand(&mut g, 0, water);
    let events = act(
        &mut g,
        0,
        Action::PlayProperty {
            card: water,
            color: Color::Utility,
            set: Some(util),
        },
    );
    assert_eq!(
        events.last(),
        Some(&Event::GameWon {
            winner: PlayerId(0)
        })
    );
    assert_eq!(g.winner(), Some(PlayerId(0)));
    assert!(g.pending_actors().is_empty());
    assert_eq!(
        g.apply(PlayerId(0), Action::EndTurn),
        Err(ActionError::GameOver)
    );
}

#[test]
fn win_completed_on_another_turn_waits_for_own_turn() {
    let mut g = setup(2);
    full_dark_blue(&mut g, 1);
    full_brown(&mut g, 1);
    to_set(&mut g, 1, Color::Utility, &["Electric Company"]);
    to_set(&mut g, 1, Color::Red, &["Kentucky Avenue"]);
    let water = named(&mut g, "Water Works");
    to_set_cards(&mut g, 0, Color::Utility, vec![water]);
    let forced = action_card(&mut g, ActionKind::ForcedDeal);
    to_hand(&mut g, 0, forced);
    let kentucky = g.players[1].sets[3].cards[0];
    act(
        &mut g,
        0,
        Action::PlayForcedDeal {
            card: forced,
            give: water,
            take: kentucky,
        },
    );
    act(&mut g, 1, Action::Accept);
    assert_eq!(g.complete_colors(PlayerId(1)), 3);
    assert_eq!(g.winner(), None, "player 1 must wait for their own turn");
    let draw_before = g.draw_pile.len();
    act(&mut g, 0, Action::EndTurn);
    assert_eq!(g.winner(), Some(PlayerId(1)));
    assert_eq!(
        g.draw_pile.len(),
        draw_before,
        "win is checked before drawing"
    );
}

// ---------- random play ----------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn check_invariants(g: &Game) {
    assert_conserved(g);
    for p in &g.players {
        for s in &p.sets {
            assert!(s.cards.len() <= s.color.set_size());
            assert!(s.hotel.is_none() || s.house.is_some());
            assert!(!s.cards.is_empty() || s.house.is_some());
            for &c in &s.cards {
                assert!(card_def(c).property_colors().contains(&s.color));
            }
        }
    }
    let pending = g.pending_actors();
    assert!(pending.len() <= 1);
    assert_eq!(pending.is_empty(), g.winner().is_some());
    for p in 0..g.num_players() {
        let id = PlayerId(p as u8);
        let legal = g.legal_actions(id);
        assert_eq!(legal.is_empty(), !pending.contains(&id));
        let obs = g.observation(Some(id));
        assert_eq!(obs.hand, g.players[p].hand);
        for (i, other) in obs.players.iter().enumerate() {
            assert_eq!(other.hand_count, g.players[i].hand.len());
        }
    }
}

#[test]
fn random_games_keep_invariants() {
    let mut finished = 0;
    for (seed, players) in [
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 2),
        (7, 3),
        (8, 8),
        (9, 2),
        (10, 4),
    ] {
        let mut g = Game::new(GameConfig::new(players, seed)).unwrap();
        let mut rng = Rng(seed * 7919 + 1);
        for step in 0..4000 {
            check_invariants(&g);
            let Some(p) = g.to_act() else { break };
            let legal = g.legal_actions(p);
            // Prefer plays over ending the turn so games progress.
            let choice = if legal.len() > 1 && !rng.next().is_multiple_of(10) {
                let non_end: Vec<&Action> =
                    legal.iter().filter(|a| **a != Action::EndTurn).collect();
                non_end[(rng.next() % non_end.len() as u64) as usize].clone()
            } else {
                legal[(rng.next() % legal.len() as u64) as usize].clone()
            };
            g.apply(p, choice).unwrap();
            if step % 500 == 0 {
                let back: Game = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
                assert_eq!(back, g);
            }
        }
        if g.winner().is_some() {
            finished += 1;
        }
    }
    assert!(
        finished >= 5,
        "most random games should end with a winner, got {finished}"
    );
}

#[test]
fn max_players_game_runs() {
    let mut g = Game::new(GameConfig::new(42, 3)).unwrap();
    for _ in 0..200 {
        let Some(p) = g.to_act() else { break };
        let a = g.legal_actions(p)[0].clone();
        g.apply(p, a).unwrap();
    }
    check_invariants(&g);
}
