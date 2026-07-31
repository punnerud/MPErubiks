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
    }
}
