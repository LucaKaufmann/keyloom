//! Physical key names (spec 01): `L.home.4`, `R.thumb.1`, … and key selectors.
//!
//! Voyager key order (docs/oryx-encodings.md): 0–23 left half, 4 rows × 6 from the OUTER edge; 24–25 left
//! thumbs; 26–49 right half, 4 rows × 6 from the INNER edge; 50–51 right thumbs. Columns in names
//! count from the outer edge on both halves, so they map to fingers.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Half {
    L,
    R,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Row {
    Num,
    Top,
    Home,
    Bot,
    Thumb,
}

impl Row {
    pub fn name(self) -> &'static str {
        match self {
            Row::Num => "num",
            Row::Top => "top",
            Row::Home => "home",
            Row::Bot => "bot",
            Row::Thumb => "thumb",
        }
    }
    const MAIN: [Row; 4] = [Row::Num, Row::Top, Row::Home, Row::Bot];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Finger {
    Pinky,
    Ring,
    Middle,
    Index,
    Thumb,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Position {
    pub index: usize,
    pub name: String,
    pub half: Half,
    pub row: Row,
    /// Main rows: 0..5 from the outer edge. Thumbs: 1 = under the main block, 2 = toward the centre.
    pub col: u8,
    pub finger: Finger,
    /// 0 (easiest) to 1 (hardest), hand-tuned (spec 07).
    pub effort: f32,
    /// Home-row key under a resting finger.
    pub home: bool,
}

impl Position {
    /// Human description for guides, e.g. "left home row, 2nd from the outer edge".
    pub fn describe(&self) -> String {
        let half = match self.half {
            Half::L => "left",
            Half::R => "right",
        };
        if self.row == Row::Thumb {
            let which = if self.col == 1 { "next to the main block" } else { "toward the centre" };
            return format!("{half} thumb key {which}");
        }
        let row = match self.row {
            Row::Num => "number row",
            Row::Top => "top row",
            Row::Home => "home row",
            Row::Bot => "bottom row",
            Row::Thumb => unreachable!(),
        };
        let n = self.col + 1;
        let suffix = match n {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        };
        format!("{half} {row}, {n}{suffix} from the outer edge")
    }
}

/// Finger and effort for a main-block key (col from the outer edge).
fn ergonomics(row: Row, col: u8) -> (Finger, f32, bool) {
    if row == Row::Thumb {
        return (Finger::Thumb, if col == 1 { 0.05 } else { 0.15 }, false);
    }
    let (finger, base) = match col {
        0 => (Finger::Pinky, 0.55),
        1 => (Finger::Pinky, 0.3),
        2 => (Finger::Ring, 0.2),
        3 => (Finger::Middle, 0.05),
        4 => (Finger::Index, 0.0),
        _ => (Finger::Index, 0.15),
    };
    let row_add = match row {
        Row::Home => 0.0,
        Row::Top => 0.15,
        Row::Bot => 0.2,
        _ => 0.4,
    };
    let effort: f32 = base + row_add;
    (finger, (effort.min(1.0) * 100.0).round() / 100.0, row == Row::Home && (1..=4).contains(&col))
}

#[derive(Debug, Clone)]
pub struct PositionMap {
    pub geometry: String,
    pub positions: Vec<Position>,
}

fn pos(index: usize, half: Half, row: Row, col: u8) -> Position {
    let h = match half {
        Half::L => "L",
        Half::R => "R",
    };
    let (finger, effort, home) = ergonomics(row, col);
    Position { index, name: format!("{h}.{}.{col}", row.name()), half, row, col, finger, effort, home }
}

impl PositionMap {
    /// Position names for a geometry, if keyloom knows it (Voyager today).
    pub fn for_geometry(geometry: &str) -> Option<Self> {
        match geometry {
            "voyager" => Some(Self::voyager()),
            _ => None,
        }
    }

    pub fn voyager() -> Self {
        let mut positions = Vec::with_capacity(52);
        for i in 0..24 {
            positions.push(pos(i, Half::L, Row::MAIN[i / 6], (i % 6) as u8));
        }
        positions.push(pos(24, Half::L, Row::Thumb, 1));
        positions.push(pos(25, Half::L, Row::Thumb, 2));
        for i in 26..50 {
            let j = i - 26;
            positions.push(pos(i, Half::R, Row::MAIN[j / 6], 5 - (j % 6) as u8));
        }
        positions.push(pos(50, Half::R, Row::Thumb, 2));
        positions.push(pos(51, Half::R, Row::Thumb, 1));
        Self { geometry: "voyager".into(), positions }
    }

    pub fn get(&self, index: usize) -> Option<&Position> {
        self.positions.get(index)
    }

    pub fn by_name(&self, name: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn name(&self, index: usize) -> String {
        self.get(index).map(|p| p.name.clone()).unwrap_or_else(|| format!("#{index}"))
    }

    pub fn key_count(&self) -> usize {
        self.positions.len()
    }
}

/// Name of a key for display, also for geometries without a position map.
pub fn display_name(map: Option<&PositionMap>, index: usize) -> String {
    map.map(|m| m.name(index)).unwrap_or_else(|| format!("#{index}"))
}

/// Parses a key selector to an index: a position name (`L.home.4`) or an index (`#17`).
/// Legend (`key:F`) and keycode (`code:…`) selectors come with the explore queries.
pub fn parse_key(map: Option<&PositionMap>, input: &str, key_count: usize) -> Result<usize, String> {
    let s = input.trim();
    if let Some(n) = s.strip_prefix('#') {
        let i: usize = n.parse().map_err(|_| format!("'{s}' is not a key index"))?;
        return if i < key_count { Ok(i) } else { Err(format!("key #{i} doesn't exist (0–{})", key_count - 1)) };
    }
    map.and_then(|m| m.by_name(s))
        .map(|p| p.index)
        .ok_or_else(|| format!("unknown key '{s}' (use a position like L.home.4 or an index like #17)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voyager_names() {
        let m = PositionMap::voyager();
        assert_eq!(m.key_count(), 52);
        let mut names: Vec<_> = m.positions.iter().map(|p| p.name.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 52, "names are unique");
        for (i, p) in m.positions.iter().enumerate() {
            assert_eq!(p.index, i);
        }
        assert_eq!(m.name(0), "L.num.0");
        assert_eq!(m.name(16), "L.home.4"); // F on QWERTY, T on Colemak-DH
        assert_eq!(m.name(24), "L.thumb.1");
        assert_eq!(m.name(25), "L.thumb.2");
        assert_eq!(m.name(26), "R.num.5"); // inner column of the right half
        assert_eq!(m.name(31), "R.num.0");
        assert_eq!(m.name(39), "R.home.4"); // J on QWERTY
        assert_eq!(m.name(50), "R.thumb.2");
        assert_eq!(m.name(51), "R.thumb.1");
    }

    #[test]
    fn ergonomics_and_descriptions() {
        let m = PositionMap::voyager();
        let p = |n: &str| m.by_name(n).unwrap().clone();
        assert_eq!((p("L.home.4").finger, p("L.home.4").effort, p("L.home.4").home), (Finger::Index, 0.0, true));
        assert_eq!(p("R.home.1").finger, Finger::Pinky);
        assert!(p("L.num.0").effort > p("L.bot.1").effort);
        assert!(p("L.thumb.1").effort < p("L.thumb.2").effort);
        assert_eq!(p("L.home.1").describe(), "left home row, 2nd from the outer edge");
        assert_eq!(p("R.thumb.2").describe(), "right thumb key toward the centre");
    }

    #[test]
    fn selectors() {
        let m = PositionMap::voyager();
        assert_eq!(parse_key(Some(&m), "l.HOME.4", 52), Ok(16));
        assert_eq!(parse_key(Some(&m), "#51", 52), Ok(51));
        assert!(parse_key(Some(&m), "#52", 52).is_err());
        assert!(parse_key(Some(&m), "L.home.9", 52).is_err());
        assert_eq!(parse_key(None, "#3", 72), Ok(3));
    }
}
