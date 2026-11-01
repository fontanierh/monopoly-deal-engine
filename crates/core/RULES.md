# Monopoly Deal rules: 2008 edition

This file is the rules reference for this repository. It covers the printed 2008 rules and every choice the engine makes
where those rules are silent. Card values and rent ladders are defined once, in [`src/cards.rs`](src/cards.rs).

Each rule is marked:

- **Printed**: stated in the 2008 Quick Start Rules or on a 2008 card face.
- **Engine default**: the 2008 text is silent or unclear, so the engine picks a behavior. Engine defaults are not official rules.

## Sources

The 2008 Hasbro Quick Start Rules are the basis. The Quick Start does not answer every question.

| Source | Edition | Use |
|---|---|---|
| [2008 Quick Start Rules](https://web.archive.org/web/20090407120541/http://host.exemplum.com/hasbro/hasbro-card-games/pdf/monopolyDeal.pdf) (Wayback Machine copy; the original host is gone) | Hasbro 2008, 1 page | **Rules basis.** Setup, turn, payment, winning, card details, contents list. |
| [B0965 card reference](https://instructions.hasbro.com/api/download/B0965_en-us_monopoly-deal-card-game.pdf) | Hasbro US, 2014 printing of the 2008 cards | Card faces, card counts and card text. Its Property Wild Card text adds "Can also be banked as money"; the 2008 Quick Start does not say this, and the engine does not follow it. |
| [monopolydealrules.com](https://monopolydealrules.com/) | Fan site, not official | Photos of 2008 cards, used only to check property and wild card faces that the official sheets do not show. |

## Cards

- Printed: the box holds 110 cards. Four are Quick Start Rules cards and are not played. The engine models the other
  **106 playing cards**:

  | Type | Count |
  |---|---|
  | Property | 28: 2 Brown, 3 Light Blue, 3 Pink, 3 Orange, 3 Red, 3 Yellow, 3 Green, 2 Dark Blue, 4 Railroad, 2 Utility |
  | Property Wild Card | 11: 1 Light Blue/Brown, 1 Light Blue/Railroad, 2 Pink/Orange, 2 Red/Yellow, 1 Dark Blue/Green, 1 Green/Railroad, 1 Railroad/Utility, 2 multicolored |
  | Action | 34: 2 Deal Breaker, 3 Forced Deal, 3 Sly Deal, 3 Just Say No, 3 Debt Collector, 3 It's My Birthday, 2 Double the Rent, 3 House, 2 Hotel, 10 Pass Go |
  | Rent | 13: 2 each of Light Blue/Brown, Pink/Orange, Red/Yellow, Dark Blue/Green, Railroad/Utility; 3 any color |
  | Money | 20: 6 x 1M, 5 x 2M, 3 x 3M, 3 x 4M, 2 x 5M, 1 x 10M |

- The Quick Start contents list says "Blue" for the Dark Blue properties. They are the same two cards.
- Action, rent and money values come from the printed text of both official sheets. Property values, rent ladders
  and wild card values were checked against the card faces shown on the official sheets, and against fan photos of
  2008 cards for the rest. No complete official scan of the 2008 card faces was available.
- Printed: multicolored wilds have no money value and cannot be used to pay.
- Engine: card ids are fixed. Id `k` is always `PACK[k % 106]`. Shuffling changes only the order.

## Setup

- Printed: 2 to 5 players use one pack. For 6 or more players, shuffle two packs together.
- Engine default: at most 42 players, because two packs (212 cards) can deal a 5-card hand to at most 42 players. Duplicate
  cards from the second pack play like the first.
- Printed: shuffle, deal 5 cards face down to each player, and put the rest face down as the draw pile.
- Engine default: seat 0 goes first. Play passes to the next seat number (the printed rules say clockwise).
- Engine: the shuffle uses SplitMix64 seeded from `GameConfig::seed`. The same seed and the same actions give the
  same game. The RNG state is part of the serialized `Game`.

## Turn

- Printed: draw 2 cards at the start of your turn. If you have no cards left, draw 5 instead. Engine: "no cards left"
  means an empty hand at the start of the turn.
- Printed: play up to 3 cards, in any order: bank a money or action card, lay down a property, or play an action card
  into the center (the play pile). You may play none.
- Printed: at the end of your turn, discard down to 7 cards in hand. Discards go to the **bottom of the draw pile**.
  Engine: the player picks them one at a time (`discard`). They are face down, so events and observations show only
  that a card was discarded.
- Engine default: when a draw finds the draw pile empty, the play pile is shuffled to form a new draw pile. If both are empty,
  the player draws what is available. This applies to every draw: turn start and Pass Go.
- Engine default: a game ends only with a win. There is no deck-out or turn-limit ending.

## Banking

- Printed: money and action cards can be banked. Rent, House, Hotel and Double the Rent cards are action cards. A banked
  action card loses its action for the rest of the game.
- Engine default: property cards, including all wilds, cannot be banked. The Quick Start allows banking only money and
  action cards.

## Properties and sets

- Printed: each property card shows how many cards make a full set of its color. Railroads and Utilities follow the
  same rule. A wild counts as one of the colors shown on it; a multicolored wild counts as any color.
- Printed: you may reorganize your property collection only on your own turn.
- Engine default: reorganizing is free (it uses no play) and is allowed only in your own play phase. `move_property` can move a
  wild to another of its colors, or move any property to another incomplete set of its color or to a new set. Any
  property may leave a full set this way.
- Engine: each set on the table has a `SetId`. When you lay down a property you choose its set:
  `play_property { set: Some(id) }` joins that incomplete set of the color, and `set: None` starts a new set.
- Engine default: cards received from another player (payment, Sly Deal, Forced Deal) keep their color and join the receiver's
  first incomplete set of that color, or a new set. The receiver can reorganize them on their own turn.
- Engine default: a set may consist only of multicolored wilds. It is full when it has enough cards.
- Engine default: a player may hold two full sets of the same color (two packs, or extra wilds). They are separate sets for
  rent, buildings and Deal Breaker, but count as one color toward the win.

## Buildings

- Printed: a House adds 3M and a Hotel adds 4M to the rent of a full set you own. One House and one Hotel per set. A
  Hotel needs a House first. Not on Railroad or Utility sets.
- Engine default: buildings stay with their set if the set stops being full. Their rent bonus counts only while the set is
  full. A set with buildings and no cards stays on the table and can be refilled.
- Engine default: buildings cannot be used to pay and cannot move to another set.

## Action cards

- Printed: **Pass Go**: draw 2 cards. You may play more than one per turn; each uses a play.
- Printed: **Debt Collector**: one player of your choice pays you 5M.
- Printed: **It's My Birthday**: every other player pays you 2M.
- Printed: **Rent**, two colors: every other player pays you rent for your properties of one of the card's colors.
  **Rent**, any color: one player of your choice pays you rent.
- Engine default: a Rent card charges for one of your sets of a matching color, and that set must have at least one card.
  Rent is the ladder value for the number of cards in the set (capped at the full-set size), plus buildings if the set
  is full. Rent cannot be charged for a color you do not have.
- Printed: **Double the Rent** must be played with a rent card and doubles the amount. Engine default: it works with either kind
  of Rent card (the card face says "a rent card"), each Double the Rent uses one play, and two can be stacked (x4). It
  cannot be played on its own.
- Printed: **Sly Deal**: take one property from another player. It cannot be part of a full set.
- Printed: **Forced Deal**: swap one of your properties with another player's property. The card face says "Cannot be
  part of a full set." Engine default: this applies to both cards in the swap.
- Printed: **Deal Breaker**: take a full set from another player, including its buildings. Engine: the set keeps its
  `SetId`. Engine default: a Deal Breaker may target a player who holds 3 full sets, because that player has not yet won
  (see [Winning](#winning)).
- Engine default: Sly Deal, Forced Deal, Deal Breaker and any-color Rent need a valid target to be played as actions. They can
  always be banked instead.

## Just Say No

- Printed: play it to cancel an action card played against you. The player who played the action may cancel your Just
  Say No with another Just Say No.
- Engine default: chains have no length limit. The action takes effect for that target if an even number of Just Say No cards
  were played.
- Engine default: Just Say No in a response never uses a play, on either player's turn. It can be played only in a response
  window.
- Engine: every target of an action gets a response window (`accept` or `just_say_no`) whether or not they hold a Just
  Say No, so the window does not reveal their hand.
- Engine default: actions against several players (It's My Birthday, two-color Rent) resolve one target at a time, in seat order
  after the actor. Each target's Just Say No protects only that target.

## Payment

- Printed: pay only from the cards in front of you (bank and properties), never from your hand. The payer chooses the
  cards. No change is given.
- Printed: paid properties go into the receiver's property collection. Paid bank cards, including action cards, go into
  the receiver's bank and cannot be used for their action.
- Printed: a player with no cards in front of them pays nothing.
- Engine: payment is one card per `pay_card` action. It ends when the paid value reaches the amount owed, or when the
  payer has no payable cards left.
- Engine default: a payer who owes more than they have pays every payable card. Properties from full sets may be paid.

## Winning

- Printed: the first player with 3 full property sets of different colors wins.
- Printed: a player who completes 3 sets during another player's turn must wait until their own turn to say so.
- Engine default: the current player's sets are checked after every action, including payments and responses made by other
  players. That player wins at once. Other players are checked at the start of their own turn, before they draw,
  and win only if they still hold 3 full sets of different colors.

## Hidden information

- `Game` holds hands, draw order and RNG state. Store it only in trusted places.
- `Game::observation(Some(p))` shows only p's hand and p's legal actions. `observation(None)` is a spectator view with no
  hand.
- Events contain no hidden card ids: draws and discards to the draw pile show counts only. Replays need the recorded
  actions, not the event log.
