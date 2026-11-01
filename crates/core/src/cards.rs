//! The 2008 card registry. One pack holds 106 playing cards; the 4 rules cards are not modeled.

use serde::{Deserialize, Serialize};

/// Cards in one pack.
pub const PACK_SIZE: u16 = 106;

/// Unique id of one physical card in a game. Pack `k` uses ids `k * 106 .. (k + 1) * 106`,
/// in registry order, so an id means the same card in every game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Brown,
    LightBlue,
    Pink,
    Orange,
    Red,
    Yellow,
    Green,
    DarkBlue,
    Railroad,
    Utility,
}

pub const ALL_COLORS: [Color; 10] = [
    Color::Brown,
    Color::LightBlue,
    Color::Pink,
    Color::Orange,
    Color::Red,
    Color::Yellow,
    Color::Green,
    Color::DarkBlue,
    Color::Railroad,
    Color::Utility,
];

impl Color {
    /// Rent by number of properties in the set; the length is the full-set size.
    pub fn rent_ladder(self) -> &'static [u8] {
        match self {
            Color::Brown => &[1, 2],
            Color::LightBlue => &[1, 2, 3],
            Color::Pink => &[1, 2, 4],
            Color::Orange => &[1, 3, 5],
            Color::Red => &[2, 3, 6],
            Color::Yellow => &[2, 4, 6],
            Color::Green => &[2, 4, 7],
            Color::DarkBlue => &[3, 8],
            Color::Railroad => &[1, 2, 3, 4],
            Color::Utility => &[1, 2],
        }
    }

    pub fn set_size(self) -> usize {
        self.rent_ladder().len()
    }

    /// Houses and hotels cannot go on Railroad or Utility sets.
    pub fn allows_buildings(self) -> bool {
        !matches!(self, Color::Railroad | Color::Utility)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    DealBreaker,
    ForcedDeal,
    SlyDeal,
    JustSayNo,
    DebtCollector,
    ItsMyBirthday,
    DoubleTheRent,
    House,
    Hotel,
    PassGo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CardKind {
    Money,
    Property {
        color: Color,
    },
    /// Two-color wild, or the multicolored wild when `colors` holds all ten colors.
    Wild {
        colors: &'static [Color],
    },
    Action {
        action: ActionKind,
    },
    /// Two-color rent charges every other player; the multicolored "any rent" charges one player.
    Rent {
        colors: &'static [Color],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CardDef {
    pub name: &'static str,
    /// Bank value in M. Zero for the multicolored wild, which cannot be banked or paid.
    pub value: u8,
    pub kind: CardKind,
}

impl CardDef {
    pub fn is_property(&self) -> bool {
        matches!(self.kind, CardKind::Property { .. } | CardKind::Wild { .. })
    }

    /// Colors this property card may be assigned to.
    pub fn property_colors(&self) -> &'static [Color] {
        match &self.kind {
            CardKind::Property { color } => {
                let i = *color as usize;
                &ALL_COLORS[i..=i]
            }
            CardKind::Wild { colors } => colors,
            _ => &[],
        }
    }

    pub fn action(&self) -> Option<ActionKind> {
        match self.kind {
            CardKind::Action { action } => Some(action),
            _ => None,
        }
    }

    pub fn is_multicolor_wild(&self) -> bool {
        matches!(self.kind, CardKind::Wild { colors } if colors.len() == ALL_COLORS.len())
    }

    /// Money, action and rent cards can be banked. Property cards, wilds included, cannot (2008 Quick Start, play option A).
    pub fn bankable(&self) -> bool {
        !self.is_property()
    }
}

use Color::*;

const fn money(value: u8, name: &'static str) -> CardDef {
    CardDef {
        name,
        value,
        kind: CardKind::Money,
    }
}
const fn prop(color: Color, value: u8, name: &'static str) -> CardDef {
    CardDef {
        name,
        value,
        kind: CardKind::Property { color },
    }
}
const fn wild(colors: &'static [Color], value: u8, name: &'static str) -> CardDef {
    CardDef {
        name,
        value,
        kind: CardKind::Wild { colors },
    }
}
const fn action(action: ActionKind, value: u8, name: &'static str) -> CardDef {
    CardDef {
        name,
        value,
        kind: CardKind::Action { action },
    }
}
const fn rent(colors: &'static [Color], value: u8, name: &'static str) -> CardDef {
    CardDef {
        name,
        value,
        kind: CardKind::Rent { colors },
    }
}

const LB_BROWN: &[Color] = &[LightBlue, Brown];
const LB_RAIL: &[Color] = &[LightBlue, Railroad];
const PINK_ORANGE: &[Color] = &[Pink, Orange];
const RED_YELLOW: &[Color] = &[Red, Yellow];
const DB_GREEN: &[Color] = &[DarkBlue, Green];
const GREEN_RAIL: &[Color] = &[Green, Railroad];
const RAIL_UTIL: &[Color] = &[Railroad, Utility];

use ActionKind::*;

/// Registry order of one pack. Index = `CardId % 106`.
pub static PACK: [CardDef; PACK_SIZE as usize] = [
    // 20 money
    money(1, "1M"),
    money(1, "1M"),
    money(1, "1M"),
    money(1, "1M"),
    money(1, "1M"),
    money(1, "1M"),
    money(2, "2M"),
    money(2, "2M"),
    money(2, "2M"),
    money(2, "2M"),
    money(2, "2M"),
    money(3, "3M"),
    money(3, "3M"),
    money(3, "3M"),
    money(4, "4M"),
    money(4, "4M"),
    money(4, "4M"),
    money(5, "5M"),
    money(5, "5M"),
    money(10, "10M"),
    // 28 properties
    prop(Brown, 1, "Mediterranean Avenue"),
    prop(Brown, 1, "Baltic Avenue"),
    prop(LightBlue, 1, "Oriental Avenue"),
    prop(LightBlue, 1, "Vermont Avenue"),
    prop(LightBlue, 1, "Connecticut Avenue"),
    prop(Pink, 2, "St. Charles Place"),
    prop(Pink, 2, "States Avenue"),
    prop(Pink, 2, "Virginia Avenue"),
    prop(Orange, 2, "St. James Place"),
    prop(Orange, 2, "Tennessee Avenue"),
    prop(Orange, 2, "New York Avenue"),
    prop(Red, 3, "Kentucky Avenue"),
    prop(Red, 3, "Indiana Avenue"),
    prop(Red, 3, "Illinois Avenue"),
    prop(Yellow, 3, "Atlantic Avenue"),
    prop(Yellow, 3, "Ventnor Avenue"),
    prop(Yellow, 3, "Marvin Gardens"),
    prop(Green, 4, "Pacific Avenue"),
    prop(Green, 4, "North Carolina Avenue"),
    prop(Green, 4, "Pennsylvania Avenue"),
    prop(DarkBlue, 4, "Park Place"),
    prop(DarkBlue, 4, "Boardwalk"),
    prop(Railroad, 2, "Reading Railroad"),
    prop(Railroad, 2, "Pennsylvania Railroad"),
    prop(Railroad, 2, "B. & O. Railroad"),
    prop(Railroad, 2, "Short Line"),
    prop(Utility, 2, "Electric Company"),
    prop(Utility, 2, "Water Works"),
    // 11 property wildcards
    wild(LB_BROWN, 1, "Property Wild Card: Light Blue/Brown"),
    wild(LB_RAIL, 4, "Property Wild Card: Light Blue/Railroad"),
    wild(PINK_ORANGE, 2, "Property Wild Card: Pink/Orange"),
    wild(PINK_ORANGE, 2, "Property Wild Card: Pink/Orange"),
    wild(RED_YELLOW, 3, "Property Wild Card: Red/Yellow"),
    wild(RED_YELLOW, 3, "Property Wild Card: Red/Yellow"),
    wild(DB_GREEN, 4, "Property Wild Card: Dark Blue/Green"),
    wild(GREEN_RAIL, 4, "Property Wild Card: Green/Railroad"),
    wild(RAIL_UTIL, 2, "Property Wild Card: Railroad/Utility"),
    wild(&ALL_COLORS, 0, "Property Wild Card: Multicolor"),
    wild(&ALL_COLORS, 0, "Property Wild Card: Multicolor"),
    // 34 action cards
    action(DealBreaker, 5, "Deal Breaker"),
    action(DealBreaker, 5, "Deal Breaker"),
    action(ForcedDeal, 3, "Forced Deal"),
    action(ForcedDeal, 3, "Forced Deal"),
    action(ForcedDeal, 3, "Forced Deal"),
    action(SlyDeal, 3, "Sly Deal"),
    action(SlyDeal, 3, "Sly Deal"),
    action(SlyDeal, 3, "Sly Deal"),
    action(JustSayNo, 4, "Just Say No!"),
    action(JustSayNo, 4, "Just Say No!"),
    action(JustSayNo, 4, "Just Say No!"),
    action(DebtCollector, 3, "Debt Collector"),
    action(DebtCollector, 3, "Debt Collector"),
    action(DebtCollector, 3, "Debt Collector"),
    action(ItsMyBirthday, 2, "It's My Birthday"),
    action(ItsMyBirthday, 2, "It's My Birthday"),
    action(ItsMyBirthday, 2, "It's My Birthday"),
    action(DoubleTheRent, 1, "Double the Rent!"),
    action(DoubleTheRent, 1, "Double the Rent!"),
    action(House, 3, "House"),
    action(House, 3, "House"),
    action(House, 3, "House"),
    action(Hotel, 4, "Hotel"),
    action(Hotel, 4, "Hotel"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    action(PassGo, 1, "Pass Go"),
    // 13 rent cards
    rent(LB_BROWN, 1, "Rent: Light Blue/Brown"),
    rent(LB_BROWN, 1, "Rent: Light Blue/Brown"),
    rent(PINK_ORANGE, 1, "Rent: Pink/Orange"),
    rent(PINK_ORANGE, 1, "Rent: Pink/Orange"),
    rent(RED_YELLOW, 1, "Rent: Red/Yellow"),
    rent(RED_YELLOW, 1, "Rent: Red/Yellow"),
    rent(DB_GREEN, 1, "Rent: Dark Blue/Green"),
    rent(DB_GREEN, 1, "Rent: Dark Blue/Green"),
    rent(RAIL_UTIL, 1, "Rent: Railroad/Utility"),
    rent(RAIL_UTIL, 1, "Rent: Railroad/Utility"),
    rent(&ALL_COLORS, 3, "Rent: Any Color"),
    rent(&ALL_COLORS, 3, "Rent: Any Color"),
    rent(&ALL_COLORS, 3, "Rent: Any Color"),
];

/// Definition of a card id. Ids at or beyond 212 wrap modulo the pack; the engine never creates them.
pub fn card_def(id: CardId) -> &'static CardDef {
    &PACK[(id.0 % PACK_SIZE) as usize]
}
