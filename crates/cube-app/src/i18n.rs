//! i18n: one compile-checked English source of truth plus CSV
//! translations (`assets/i18n/<code>.csv`, one file per language — the
//! format agents and humans can both edit). Missing keys fall back to
//! English, so a half-finished translation degrades gracefully instead
//! of showing blanks.
//!
//! Non-Latin scripts (CJK) need glyphs the default font lacks; those
//! languages name a FONT SUBSET that the app fetches on demand
//! (`assets/fonts/<code>.ttf`, a few dozen KB containing only the
//! characters this UI actually uses) — the wasm bundle stays small no
//! matter how many scripts are supported.

use std::collections::HashMap;
use std::sync::OnceLock;

/// How to paint a language's flag (egui's fonts have no flag emoji, and
/// bitmaps for 31 flags would dwarf the app).
#[derive(Clone, Copy)]
pub enum Flag {
    /// Equal horizontal bands, top to bottom.
    HBands(&'static [u32]),
    /// Equal vertical bands, left to right.
    VBands(&'static [u32]),
    /// Nordic cross, optionally with an inner cross stripe.
    Nordic { field: u32, cross: u32, inner: Option<u32> },
    Disc { field: u32, disc: u32 },
    Star { field: u32, star: u32 },
    Triangle { top: u32, bottom: u32, tri: u32 },
    Crescent { field: u32, mark: u32 },
    Diamond { field: u32, diamond: u32, disc: u32 },
    Canton { field: u32, canton: u32, sun: u32 },
    Greek,
    Taegeuk,
    UnionJack,
}

pub struct LangDef {
    pub code: &'static str,
    /// The language's own name, as its speakers write it.
    pub native: &'static str,
    /// Latin-script name, for scripts whose glyphs arrive with a font
    /// download — the row stays readable before (and if) it lands.
    pub latin: Option<&'static str>,
    pub flag: Flag,
    /// Font subset to fetch for this language's script (None = the
    /// default font already covers it).
    pub font: Option<&'static str>,
    csv: &'static str,
}

pub static LANGS: &[LangDef] = &[
    LangDef {
        code: "en",
        latin: None,
        native: "English",
        flag: Flag::UnionJack,
        font: None,
        csv: "",
    },
    LangDef {
        code: "no",
        latin: None,
        native: "Norsk bokmål",
        flag: Flag::Nordic { field: 0xBA0C2F, cross: 0xFFFFFF, inner: Some(0x00205B) },
        font: None,
        csv: include_str!("../../../assets/i18n/no.csv"),
    },
    LangDef {
        code: "sv",
        latin: None,
        native: "Svenska",
        flag: Flag::Nordic { field: 0x006AA7, cross: 0xFECC02, inner: None },
        font: None,
        csv: include_str!("../../../assets/i18n/sv.csv"),
    },
    LangDef {
        code: "da",
        latin: None,
        native: "Dansk",
        flag: Flag::Nordic { field: 0xC8102E, cross: 0xFFFFFF, inner: None },
        font: None,
        csv: include_str!("../../../assets/i18n/da.csv"),
    },
    LangDef {
        code: "fi",
        latin: None,
        native: "Suomi",
        flag: Flag::Nordic { field: 0xFFFFFF, cross: 0x003580, inner: None },
        font: None,
        csv: include_str!("../../../assets/i18n/fi.csv"),
    },
    LangDef {
        code: "is",
        latin: None,
        native: "Íslenska",
        flag: Flag::Nordic { field: 0x02529C, cross: 0xFFFFFF, inner: Some(0xDC1E35) },
        font: None,
        csv: include_str!("../../../assets/i18n/is.csv"),
    },
    LangDef {
        code: "de",
        latin: None,
        native: "Deutsch",
        flag: Flag::HBands(&[0x000000, 0xDD0000, 0xFFCE00]),
        font: None,
        csv: include_str!("../../../assets/i18n/de.csv"),
    },
    LangDef {
        code: "fr",
        latin: None,
        native: "Français",
        flag: Flag::VBands(&[0x002395, 0xFFFFFF, 0xED2939]),
        font: None,
        csv: include_str!("../../../assets/i18n/fr.csv"),
    },
    LangDef {
        code: "es",
        latin: None,
        native: "Español",
        flag: Flag::HBands(&[0xAA151B, 0xF1BF00, 0xF1BF00, 0xAA151B]),
        font: None,
        csv: include_str!("../../../assets/i18n/es.csv"),
    },
    LangDef {
        code: "pt",
        latin: None,
        native: "Português",
        flag: Flag::VBands(&[0x046A38, 0x046A38, 0xDA291C, 0xDA291C, 0xDA291C]),
        font: None,
        csv: include_str!("../../../assets/i18n/pt.csv"),
    },
    LangDef {
        code: "pt-BR",
        latin: None,
        native: "Português (Brasil)",
        flag: Flag::Diamond { field: 0x009C3B, diamond: 0xFFDF00, disc: 0x002776 },
        font: None,
        csv: include_str!("../../../assets/i18n/pt-BR.csv"),
    },
    LangDef {
        code: "it",
        latin: None,
        native: "Italiano",
        flag: Flag::VBands(&[0x008C45, 0xF4F5F0, 0xCD212A]),
        font: None,
        csv: include_str!("../../../assets/i18n/it.csv"),
    },
    LangDef {
        code: "nl",
        latin: None,
        native: "Nederlands",
        flag: Flag::HBands(&[0xAE1C28, 0xFFFFFF, 0x21468B]),
        font: None,
        csv: include_str!("../../../assets/i18n/nl.csv"),
    },
    LangDef {
        code: "pl",
        latin: None,
        native: "Polski",
        flag: Flag::HBands(&[0xFFFFFF, 0xDC143C]),
        font: None,
        csv: include_str!("../../../assets/i18n/pl.csv"),
    },
    LangDef {
        code: "cs",
        latin: None,
        native: "Čeština",
        flag: Flag::Triangle { top: 0xFFFFFF, bottom: 0xD7141A, tri: 0x11457E },
        font: None,
        csv: include_str!("../../../assets/i18n/cs.csv"),
    },
    LangDef {
        code: "sk",
        latin: None,
        native: "Slovenčina",
        flag: Flag::HBands(&[0xFFFFFF, 0x0B4EA2, 0xEE1C25]),
        font: None,
        csv: include_str!("../../../assets/i18n/sk.csv"),
    },
    LangDef {
        code: "hu",
        latin: None,
        native: "Magyar",
        flag: Flag::HBands(&[0xCE2939, 0xFFFFFF, 0x477050]),
        font: None,
        csv: include_str!("../../../assets/i18n/hu.csv"),
    },
    LangDef {
        code: "ro",
        latin: None,
        native: "Română",
        flag: Flag::VBands(&[0x002B7F, 0xFCD116, 0xCE1126]),
        font: None,
        csv: include_str!("../../../assets/i18n/ro.csv"),
    },
    LangDef {
        code: "el",
        latin: None,
        native: "Ελληνικά",
        flag: Flag::Greek,
        font: None,
        csv: include_str!("../../../assets/i18n/el.csv"),
    },
    LangDef {
        code: "tr",
        latin: None,
        native: "Türkçe",
        flag: Flag::Crescent { field: 0xE30A17, mark: 0xFFFFFF },
        font: None,
        csv: include_str!("../../../assets/i18n/tr.csv"),
    },
    LangDef {
        code: "uk",
        latin: None,
        native: "Українська",
        flag: Flag::HBands(&[0x0057B7, 0xFFDD00]),
        font: None,
        csv: include_str!("../../../assets/i18n/uk.csv"),
    },
    LangDef {
        code: "ru",
        latin: None,
        native: "Русский",
        flag: Flag::HBands(&[0xFFFFFF, 0x0039A6, 0xD52B1E]),
        font: None,
        csv: include_str!("../../../assets/i18n/ru.csv"),
    },
    LangDef {
        code: "bg",
        latin: None,
        native: "Български",
        flag: Flag::HBands(&[0xFFFFFF, 0x00966E, 0xD62612]),
        font: None,
        csv: include_str!("../../../assets/i18n/bg.csv"),
    },
    LangDef {
        code: "hr",
        latin: None,
        native: "Hrvatski",
        flag: Flag::HBands(&[0xFF0000, 0xFFFFFF, 0x171796]),
        font: None,
        csv: include_str!("../../../assets/i18n/hr.csv"),
    },
    LangDef {
        code: "sl",
        latin: None,
        native: "Slovenščina",
        flag: Flag::HBands(&[0xFFFFFF, 0x0000FF, 0xFF0000]),
        font: None,
        csv: include_str!("../../../assets/i18n/sl.csv"),
    },
    LangDef {
        code: "ca",
        latin: None,
        native: "Català",
        flag: Flag::HBands(&[0xFCDD09, 0xDA121A, 0xFCDD09, 0xDA121A, 0xFCDD09, 0xDA121A, 0xFCDD09, 0xDA121A, 0xFCDD09]),
        font: None,
        csv: include_str!("../../../assets/i18n/ca.csv"),
    },
    LangDef {
        code: "zh-Hans",
        latin: Some("Chinese (Simplified)"),
        native: "简体中文",
        flag: Flag::Star { field: 0xEE1C25, star: 0xFFFF00 },
        font: Some("zh-Hans"),
        csv: include_str!("../../../assets/i18n/zh-Hans.csv"),
    },
    LangDef {
        code: "zh-Hant",
        latin: Some("Chinese (Traditional)"),
        native: "繁體中文",
        flag: Flag::Canton { field: 0xFE0000, canton: 0x000095, sun: 0xFFFFFF },
        font: Some("zh-Hant"),
        csv: include_str!("../../../assets/i18n/zh-Hant.csv"),
    },
    LangDef {
        code: "ja",
        latin: Some("Japanese"),
        native: "日本語",
        flag: Flag::Disc { field: 0xFFFFFF, disc: 0xBC002D },
        font: Some("ja"),
        csv: include_str!("../../../assets/i18n/ja.csv"),
    },
    LangDef {
        code: "ko",
        latin: Some("Korean"),
        native: "한국어",
        flag: Flag::Taegeuk,
        font: Some("ko"),
        csv: include_str!("../../../assets/i18n/ko.csv"),
    },
    LangDef {
        code: "vi",
        latin: Some("Vietnamese"),
        native: "Tiếng Việt",
        flag: Flag::Star { field: 0xDA251D, star: 0xFFFF00 },
        font: Some("vi"),
        csv: include_str!("../../../assets/i18n/vi.csv"),
    },
];

/// A language, by index into [`LANGS`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Lang(pub u8);

impl Lang {
    pub const EN: Lang = Lang(0);
    pub const NO: Lang = Lang(1);

    pub fn def(self) -> &'static LangDef {
        &LANGS[usize::from(self.0).min(LANGS.len() - 1)]
    }
    pub fn code(self) -> &'static str {
        self.def().code
    }
    pub fn from_code(code: &str) -> Option<Lang> {
        LANGS
            .iter()
            .position(|d| d.code == code)
            .map(|i| Lang(i as u8))
    }
}

macro_rules! text_keys {
    ($($key:ident => $en:expr,)*) => {
        /// Every user-visible string in the app. Exhaustive matching
        /// keeps English complete by construction.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum TextKey { $($key),* }

        impl TextKey {
            /// The key's name as written in the CSV files.
            pub fn name(self) -> &'static str {
                match self { $(TextKey::$key => stringify!($key)),* }
            }
        }

        fn en(k: TextKey) -> &'static str {
            match k { $(TextKey::$key => $en),* }
        }

        /// Every key, for exhaustiveness checks in tests and tooling.
        pub const ALL_KEYS: &[TextKey] = &[$(TextKey::$key),*];
    };
}

text_keys! {
    Practice => "Practice",
    SettingsTitle => "Solution settings",
    HintModeFull => "Full guide",
    HintModePractice => "Practice stop",
    HintModeOff => "Off",
    MaxExtraMoves => "Max extra moves",
    IDidIt => "I did it!",
    NeededHelp => "Needed a peek",
    PracticeYouKnow => "You know this one!",
    SettingsNoTrained => "Mark algorithms with the star in Train first",
    TapSelectTitle => "Tap selects",
    TapSelectSide => "The side",
    TapSelectCell => "The cell",
    MyWay => "My way",
    PracticeShuffle => "Practice shuffle",
    Moves => "moves",
    ShareOfSolution => "of the solution",
    PickAlgorithms => "Pick algorithms to drill",
    ScrambleModeRandom => "Real",
    ScrambleModeBuilt => "Built",
    ScrambleModeAuto => "Auto",
    HowManyTimes => "How many times",
    PlayAffectsGuide => "Chosen here = preferred in the solve guide too",
    AppTitle => "Rubik's Cube",
    MenuSolve => "Solve",
    MenuTrain => "Train",
    MenuPlay => "Play",
    Back => "Back",
    Scramble => "Scramble",
    Reset => "Reset",
    Undo => "Undo",
    ComingSoon => "Coming soon!",
    Play => "Play",
    Pause => "Pause",
    Next => "Next",
    GuideDone => "Solved! Great job!",
    TableLoading => "Loading solver…",
    ErrColorCounts => "Some colors have too many or too few stickers",
    ErrCenters => "The six middle stickers must all be different colors",
    ErrImpossiblePiece => "One piece has colors that don't belong together",
    ErrUnsolvable => "Almost! One sticker looks wrong — check again",
    ShowSolution => "Show me",
    Best => "Best",
    Average => "Avg (5)",
    Attempts => "Tries",
    CameraStarting => "Starting camera…",
    EnterManually => "Type it in",
    Capture => "Snap!",
    ScanHoldSteady => "Hold the cube steady in the square",
    ScanTurnLeft => "Turn the whole cube to the left",
    ScanTiltUp => "Tilt the top AWAY from you",
    LessonsTabCaption => "Start here",
    Replay => "Again!",
    LessonCubeTitle => "Meet the cube",
    LessonCubeCenters => "The middles never move — they show each side's color",
    LessonCubeSides => "Spin the whole cube — the middles still decide",
    LessonCubePieces => "Twist, and the pieces travel — watch them come home!",
    LessonMovesTitle => "Reading moves",
    LessonMovesR => "R means: turn the right side once, like a clock",
    LessonMovesPrime => "R' means: the other way",
    LessonMovesDouble => "R2 means: twice",
    LessonMovesAll => "Every side has its own letter: U F L D …",
    LessonDaisyTitle => "The daisy",
    LessonDaisyGoal => "Make a flower: white petals around the yellow middle",
    LessonDaisyFind => "Find white edges and lift them up",
    LessonCrossTitle => "The white cross",
    LessonCrossTurnDown => "Turn each petal down twice — flower becomes cross",
    LessonCrossDone => "Match the petal's side color before you turn",
    LessonCornersTitle => "White corners",
    LessonCornersMagic => "The magic move: R' D' R D",
    LessonCornersRepeat => "Repeat it until the corner sits right",
    LessonMiddleTitle => "The middle layer",
    LessonMiddleRight => "Send the edge to the right",
    LessonMiddleLeft => "Send the edge to the left",
    LessonTopTitle => "The top",
    LessonTopCross => "Yellow cross: F R U R' U' F'",
    LessonTopNext => "Now try the ABC tab — you're almost there!",
}

/// Parsed CSVs, one map per language (built once, on first use).
fn tables() -> &'static Vec<HashMap<&'static str, &'static str>> {
    static TABLES: OnceLock<Vec<HashMap<&'static str, &'static str>>> = OnceLock::new();
    TABLES.get_or_init(|| LANGS.iter().map(|d| parse_csv(d.csv)).collect())
}

/// Minimal RFC 4180 reader for `key,text` files. Quoted fields may
/// contain commas, doubled quotes and newlines. Unescaped values are
/// leaked so lookups can hand out `&'static str` (the table lives for
/// the whole program anyway).
fn parse_csv(src: &'static str) -> HashMap<&'static str, &'static str> {
    let mut out = HashMap::new();
    let bytes = src.as_bytes();
    let mut i = 0usize;
    let mut row: Vec<&'static str> = Vec::new();
    let mut first_row = true;
    while i < bytes.len() {
        // --- one field ---
        let field: &'static str;
        if bytes[i] == b'"' {
            i += 1;
            let start = i;
            let mut escaped = false;
            while i < bytes.len() {
                if bytes[i] == b'"' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'"' {
                        escaped = true;
                        i += 2;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            let raw = &src[start..i.min(src.len())];
            field = if escaped {
                Box::leak(raw.replace("\"\"", "\"").into_boxed_str())
            } else {
                raw
            };
            i += 1; // closing quote
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b',' && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            field = &src[start..i];
        }
        row.push(field);
        // --- separator ---
        if i < bytes.len() && bytes[i] == b',' {
            i += 1;
            continue;
        }
        while i < bytes.len() && (bytes[i] == b'\r' || bytes[i] == b'\n') {
            i += 1;
        }
        if !first_row {
            if let (Some(k), Some(v)) = (row.first(), row.get(1)) {
                if !k.is_empty() {
                    out.insert(*k, *v);
                }
            }
        }
        first_row = false;
        row.clear();
    }
    out
}

#[derive(Clone, Copy)]
pub struct I18n {
    pub lang: Lang,
}

impl I18n {
    /// The string for `k`, in the current language, falling back to
    /// English for keys a translation has not covered.
    pub fn t(&self, k: TextKey) -> &'static str {
        if self.lang == Lang::EN {
            return en(k);
        }
        tables()
            .get(usize::from(self.lang.0))
            .and_then(|m| m.get(k.name()).copied())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| en(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_translates_every_key() {
        let keys: Vec<TextKey> = ALL_KEYS.to_vec();
        let mut gaps = Vec::new();
        for (i, def) in LANGS.iter().enumerate() {
            if def.code == "en" {
                continue;
            }
            let i18n = I18n { lang: Lang(i as u8) };
            let missing: Vec<&str> = keys
                .iter()
                .filter(|&&k| {
                    tables()[i]
                        .get(k.name())
                        .map(|s| s.is_empty())
                        .unwrap_or(true)
                })
                .map(|k| k.name())
                .collect();
            if !missing.is_empty() {
                gaps.push(format!("{}: {} missing ({:?}…)", def.code, missing.len(), &missing[..missing.len().min(3)]));
            }
            // Fallback must never yield an empty string.
            assert!(keys.iter().all(|&k| !i18n.t(k).is_empty()), "{} has empty text", def.code);
        }
        assert!(gaps.is_empty(), "incomplete translations:\n{}", gaps.join("\n"));
    }

    #[test]
    fn quoted_csv_fields_round_trip() {
        let table = parse_csv("key,text\nA,plain\nB,\"has, comma\"\nC,\"say \"\"hi\"\"\"\n");
        assert_eq!(table.get("A").copied(), Some("plain"));
        assert_eq!(table.get("B").copied(), Some("has, comma"));
        assert_eq!(table.get("C").copied(), Some("say \"hi\""));
    }

    #[test]
    fn language_codes_are_unique_and_resolvable() {
        let mut codes: Vec<&str> = LANGS.iter().map(|d| d.code).collect();
        codes.sort_unstable();
        let n = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), n, "duplicate language codes");
        for (i, d) in LANGS.iter().enumerate() {
            assert_eq!(Lang::from_code(d.code), Some(Lang(i as u8)));
            assert!(!d.native.is_empty());
        }
    }
}
