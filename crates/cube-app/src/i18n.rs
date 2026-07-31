//! Tiny compile-checked i18n: an exhaustive key enum and one match per
//! language. Text is always SECONDARY to visuals in this app (kids who
//! can't read yet must be able to use it) — keep strings short.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    No,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextKey {
    AppTitle,
    MenuSolve,
    MenuTrain,
    MenuPlay,
    Back,
    Scramble,
    Reset,
    Undo,
    ComingSoon,
    Play,
    Pause,
    Next,
    GuideDone,
    TableLoading,
    ErrColorCounts,
    ErrCenters,
    ErrImpossiblePiece,
    ErrUnsolvable,
    ShowSolution,
    Best,
    Average,
    Attempts,
    CameraStarting,
    EnterManually,
    Capture,
    ScanHoldSteady,
    ScanTurnLeft,
    ScanTiltUp,
    ScanTiltDown,
}

#[derive(Clone, Copy)]
pub struct I18n {
    pub lang: Lang,
}

impl I18n {
    pub fn t(&self, k: TextKey) -> &'static str {
        match self.lang {
            Lang::En => en(k),
            Lang::No => no(k),
        }
    }
}

fn en(k: TextKey) -> &'static str {
    use TextKey::*;
    match k {
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
        ScanTiltUp => "Turn left once more, then tilt the top toward the camera",
        ScanTiltDown => "Tilt the cube back down twice",
    }
}

fn no(k: TextKey) -> &'static str {
    use TextKey::*;
    match k {
        AppTitle => "Rubiks kube",
        MenuSolve => "Løs",
        MenuTrain => "Tren",
        MenuPlay => "Lek",
        Back => "Tilbake",
        Scramble => "Bland",
        Reset => "Nullstill",
        Undo => "Angre",
        ComingSoon => "Kommer snart!",
        Play => "Spill av",
        Pause => "Pause",
        Next => "Neste",
        GuideDone => "Løst! Kjempebra!",
        TableLoading => "Laster løseren…",
        ErrColorCounts => "Noen farger har for mange eller for få klistremerker",
        ErrCenters => "De seks midt-klistremerkene må ha ulike farger",
        ErrImpossiblePiece => "En brikke har farger som ikke hører sammen",
        ErrUnsolvable => "Nesten! Ett klistremerke ser feil ut — sjekk igjen",
        ShowSolution => "Vis meg",
        Best => "Beste",
        Average => "Snitt (5)",
        Attempts => "Forsøk",
        CameraStarting => "Starter kameraet…",
        EnterManually => "Skriv inn selv",
        Capture => "Knips!",
        ScanHoldSteady => "Hold kuben rolig i ruten",
        ScanTurnLeft => "Snu hele kuben mot venstre",
        ScanTiltUp => "Snu venstre en gang til, så vipp toppen mot kameraet",
        ScanTiltDown => "Vipp kuben ned igjen to ganger",
    }
}
