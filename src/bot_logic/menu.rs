use std::fmt::Formatter;
use std::str::FromStr;

pub enum LocationQuestion {
    Correct,
    NumberFalse,
    AllFalse,
}

const CORRECT: &str = "Ja, beides stimmt!";
const NUMBER_FALSE: &str = "Nein, die Hausnummer stimmt nicht!";
const ALL_FALSE: &str = "Nein, beides ist falsch!";

impl std::fmt::Display for LocationQuestion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            self::LocationQuestion::Correct => write!(f, "{}", CORRECT),
            self::LocationQuestion::NumberFalse => {
                write!(f, "{}", NUMBER_FALSE)
            }
            self::LocationQuestion::AllFalse => write!(f, "{}", ALL_FALSE),
        }
    }
}

impl FromStr for LocationQuestion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            CORRECT => Ok(Self::Correct),
            NUMBER_FALSE => Ok(Self::NumberFalse),
            ALL_FALSE => Ok(Self::AllFalse),
            _ => Err(format!("Could not convert to LocationQuestion: {}", s)),
        }
    }
}

pub enum MainMenuQuestion {
    Search,
    ToggleNotifications,
    Delete,
}

const SEARCH: &str = "Straße auswählen/ändern";
const NOTIFICATION: &str = "Benachrichtigungen ein-/ausschalten";
const DELETE: &str = "Alle Daten löschen";

impl std::fmt::Display for MainMenuQuestion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            self::MainMenuQuestion::Search => {
                write!(f, "{}", SEARCH)
            }
            self::MainMenuQuestion::ToggleNotifications => {
                write!(f, "{}", NOTIFICATION)
            }
            self::MainMenuQuestion::Delete => {
                write!(f, "{}", DELETE)
            }
        }
    }
}

impl FromStr for MainMenuQuestion {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            SEARCH => Ok(MainMenuQuestion::Search),
            NOTIFICATION => Ok(MainMenuQuestion::ToggleNotifications),
            DELETE => Ok(MainMenuQuestion::Delete),
            _ => Err("Could not convert to MainMenuQuestion."),
        }
    }
}
