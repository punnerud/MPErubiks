//! Super-simple beginner lessons: each step is an ANIMATION on the 3D cube
//! plus one short sentence. Pre-readers can follow the cube alone; the
//! text is reinforcement, never the carrier.

use crate::i18n::TextKey;

pub struct Lesson {
    pub id: &'static str,
    /// Big single-character label for the card (visual, language-free).
    pub badge: &'static str,
    pub title: TextKey,
    pub steps: &'static [Step],
}

pub struct Step {
    pub text: TextKey,
    /// Applied instantly before the demo (orientation/state setup).
    pub setup: Option<&'static str>,
    /// Animated on entry and on replay.
    pub demo: Option<&'static str>,
}

pub const LESSONS: &[Lesson] = &[
    Lesson {
        id: "intro-cube",
        badge: "1",
        title: TextKey::LessonCubeTitle,
        steps: &[
            Step {
                text: TextKey::LessonCubeCenters,
                setup: Some(""),
                demo: Some("y y y y"),
            },
            Step {
                text: TextKey::LessonCubeSides,
                setup: Some(""),
                demo: Some("x y x' y'"),
            },
            Step {
                text: TextKey::LessonCubePieces,
                setup: Some(""),
                demo: Some("(R U R' U')6"),
            },
        ],
    },
    Lesson {
        id: "intro-moves",
        badge: "2",
        title: TextKey::LessonMovesTitle,
        steps: &[
            Step {
                text: TextKey::LessonMovesR,
                setup: Some(""),
                demo: Some("R R' R R'"),
            },
            Step {
                text: TextKey::LessonMovesPrime,
                setup: Some(""),
                demo: Some("R' R R' R"),
            },
            Step {
                text: TextKey::LessonMovesDouble,
                setup: Some(""),
                demo: Some("R2 R2"),
            },
            Step {
                text: TextKey::LessonMovesAll,
                setup: Some(""),
                demo: Some("U U' F F' L L' D D'"),
            },
        ],
    },
    Lesson {
        id: "intro-daisy",
        badge: "3",
        title: TextKey::LessonDaisyTitle,
        steps: &[
            Step {
                // White down, yellow up; petals bloom one by one.
                text: TextKey::LessonDaisyGoal,
                setup: Some("x2"),
                demo: Some("F2 R2 B2 L2"),
            },
            Step {
                text: TextKey::LessonDaisyFind,
                setup: Some("x2 F2 R2 B2"),
                demo: Some("L2"),
            },
        ],
    },
    Lesson {
        id: "intro-cross",
        badge: "4",
        title: TextKey::LessonCrossTitle,
        steps: &[
            Step {
                // From the daisy: each petal turns down to build the cross.
                text: TextKey::LessonCrossTurnDown,
                setup: Some("x2 F2 R2 B2 L2"),
                demo: Some("F2 R2 B2 L2"),
            },
            Step {
                text: TextKey::LessonCrossDone,
                setup: Some("x2 F2"),
                demo: Some("F2"),
            },
        ],
    },
    Lesson {
        id: "intro-corners",
        badge: "5",
        title: TextKey::LessonCornersTitle,
        steps: &[
            Step {
                text: TextKey::LessonCornersMagic,
                setup: Some("x2 D' R' D R D' R' D R"),
                demo: Some("R' D' R D R' D' R D"),
            },
            Step {
                text: TextKey::LessonCornersRepeat,
                setup: Some("x2 (D' R' D R)4"),
                demo: Some("(R' D' R D)4"),
            },
        ],
    },
    Lesson {
        id: "intro-middle",
        badge: "6",
        title: TextKey::LessonMiddleTitle,
        steps: &[
            Step {
                text: TextKey::LessonMiddleRight,
                setup: Some("x2 F' U' F U R U R' U'"),
                demo: Some("U R U' R' U' F' U F"),
            },
            Step {
                text: TextKey::LessonMiddleLeft,
                setup: Some("x2 F U F' U' L' U' L U"),
                demo: Some("U' L' U L U F U' F'"),
            },
        ],
    },
    Lesson {
        id: "intro-top",
        badge: "7",
        title: TextKey::LessonTopTitle,
        steps: &[
            Step {
                text: TextKey::LessonTopCross,
                setup: Some("x2 F U R U' R' F'"),
                demo: Some("F R U R' U' F'"),
            },
            Step {
                text: TextKey::LessonTopNext,
                setup: Some("x2 R U2 R' U' R U' R'"),
                demo: Some("R U R' U R U2 R'"),
            },
        ],
    },
];
