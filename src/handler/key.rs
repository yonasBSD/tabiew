use std::{collections::HashMap, fmt::Debug};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::AppContext, handler::action::AppAction};

enum Action {
    Direct(AppAction),
    Closure(Box<dyn Fn(KeyEvent) -> AppAction>),
}

impl Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct(arg0) => f.debug_tuple("Direct").field(arg0).finish(),
            Self::Closure(_arg0) => f.debug_tuple("Closure").finish(),
        }
    }
}

impl From<AppAction> for Action {
    fn from(value: AppAction) -> Self {
        Action::Direct(value)
    }
}

impl<F: Fn(KeyEvent) -> AppAction + 'static> From<F> for Action {
    fn from(value: F) -> Self {
        Action::Closure(Box::new(value))
    }
}

#[derive(Debug)]
struct Keybind {
    code: KeyCode,
    modifiers: KeyModifiers,
    action: Action,
}

impl Default for Keybind {
    fn default() -> Self {
        Self {
            code: KeyCode::Null,
            modifiers: KeyModifiers::empty(),
            action: Action::Direct(AppAction::NoAction),
        }
    }
}

impl Keybind {
    fn code(mut self, code: KeyCode) -> Self {
        self.code = code;
        self
    }

    fn char(mut self, c: char) -> Self {
        self.code = KeyCode::Char(c);
        if c.is_uppercase() {
            self.modifiers |= KeyModifiers::SHIFT
        }
        self
    }

    fn shift(mut self) -> Self {
        self.modifiers |= KeyModifiers::SHIFT;
        self
    }

    fn ctrl(mut self) -> Self {
        self.modifiers |= KeyModifiers::CONTROL;
        self
    }

    #[allow(dead_code)]
    fn alt(mut self) -> Self {
        self.modifiers |= KeyModifiers::ALT;
        self
    }

    #[allow(dead_code)]
    fn meta(mut self) -> Self {
        self.modifiers |= KeyModifiers::META;
        self
    }

    fn action(mut self, action: impl Into<Action>) -> Self {
        self.action = action.into();
        self
    }

    fn matches(&self, event: KeyEvent) -> Option<AppAction> {
        (self.code == event.code && self.modifiers == event.modifiers).then_some(
            match &self.action {
                Action::Direct(app_action) => app_action.clone(),
                Action::Closure(closure) => closure(event),
            },
        )
    }
}

#[derive(Default)]
struct Keybinds {
    list: Vec<Keybind>,
    fall_back: Option<Box<dyn Fn(KeyEvent) -> Option<AppAction>>>,
}

impl Keybinds {
    fn find(&self, event: KeyEvent) -> Option<AppAction> {
        self.list
            .iter()
            .find_map(|kb| kb.matches(event))
            .or(self.fall_back.as_ref().and_then(|fb| fb(event)))
    }

    fn add(&mut self, kb: Keybind) -> &mut Self {
        self.list.push(kb);
        self
    }

    fn fallback(&mut self, closure: impl Fn(KeyEvent) -> Option<AppAction> + 'static) {
        self.fall_back = Some(Box::new(closure));
    }
}

pub struct KeyHandler {
    map: HashMap<AppContext, Keybinds>,
}

impl KeyHandler {
    pub fn action(&self, mut context: AppContext, event: KeyEvent) -> AppAction {
        loop {
            if let Some(act) = self.map.get(&context).and_then(|kbl| kbl.find(event)) {
                return act;
            } else {
                if let Some(parent) = context.parent() {
                    context = parent;
                } else {
                    return AppAction::NoAction;
                }
            }
        }
    }

    fn keybinds(&mut self, context: AppContext) -> &mut Keybinds {
        self.map.entry(context).or_insert(Default::default())
    }
}

impl Default for KeyHandler {
    fn default() -> Self {
        let mut hndl = Self {
            map: Default::default(),
        };

        // ----- empty keybindings
        hndl.keybinds(AppContext::Empty)
            // q
            .add(
                Keybind::default()
                    .char('q')
                    .action(AppAction::TabRemoveOrQuit),
            )
            // shift-h shift-l shift-left shift-right
            .add(Keybind::default().char('H').action(AppAction::TabPrev))
            .add(Keybind::default().char('L').action(AppAction::TabNext))
            .add(
                Keybind::default()
                    .code(KeyCode::Left)
                    .shift()
                    .action(AppAction::TabPrev),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Right)
                    .shift()
                    .action(AppAction::TabNext),
            )
            // :
            .add(
                Keybind::default()
                    .char(':')
                    .action(AppAction::PalleteShow(String::default())),
            );

        // ----- error keybindings
        hndl.keybinds(AppContext::Error)
            .add(
                Keybind::default()
                    .char(':')
                    .action(AppAction::DismissErrorAndShowPallete),
            )
            .fallback(|_| Some(AppAction::DismissError));

        // ----- table keybindings
        hndl.keybinds(AppContext::Table)
            // enter
            .add(
                Keybind::default()
                    .code(KeyCode::Enter)
                    .action(AppAction::SheetShow),
            )
            //  /
            .add(Keybind::default().char('/').action(AppAction::SearchShow))
            //  e
            .add(
                Keybind::default()
                    .char('e')
                    .action(AppAction::TableToggleExpansion),
            )
            //  arrow keys
            .add(
                Keybind::default()
                    .code(KeyCode::Up)
                    .action(AppAction::TableGoUp(1)),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Down)
                    .action(AppAction::TableGoDown(1)),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Left)
                    .action(AppAction::TableScrollLeft),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Right)
                    .action(AppAction::TableScrollRight),
            )
            // hjkl keys
            .add(Keybind::default().char('k').action(AppAction::TableGoUp(1)))
            .add(
                Keybind::default()
                    .char('j')
                    .action(AppAction::TableGoDown(1)),
            )
            .add(
                Keybind::default()
                    .char('h')
                    .action(AppAction::TableScrollLeft),
            )
            .add(
                Keybind::default()
                    .char('l')
                    .action(AppAction::TableScrollRight),
            )
            // ctrl-u ctrl-d
            .add(
                Keybind::default()
                    .char('u')
                    .ctrl()
                    .action(AppAction::TableGoUpHalfPage),
            )
            .add(
                Keybind::default()
                    .char('d')
                    .ctrl()
                    .action(AppAction::TableGoDownHalfPage),
            )
            // ctrl-b ctrl-f pageup pagedown
            .add(
                Keybind::default()
                    .char('b')
                    .ctrl()
                    .action(AppAction::TableGoUpFullPage),
            )
            .add(
                Keybind::default()
                    .char('d')
                    .ctrl()
                    .action(AppAction::TableGoDownFullPage),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::PageUp)
                    .action(AppAction::TableGoUpFullPage),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::PageDown)
                    .action(AppAction::TableGoDownFullPage),
            )
            // _ $
            .add(
                Keybind::default()
                    .char('_')
                    .action(AppAction::TableScrollStart),
            )
            .add(
                Keybind::default()
                    .char('$')
                    .action(AppAction::TableScrollEnd),
            )
            // g G home end
            .add(
                Keybind::default()
                    .char('g')
                    .action(AppAction::TableGotoFirst),
            )
            .add(
                Keybind::default()
                    .char('G')
                    .action(AppAction::TableGotoLast),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Home)
                    .action(AppAction::TableGotoFirst),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::End)
                    .action(AppAction::TableGotoLast),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Char('r'))
                    .ctrl()
                    .action(AppAction::TableReset),
            )
            .fallback(|event| match event.code {
                KeyCode::Char('1') => Some(AppAction::PalleteShow("goto 1".to_owned())),
                KeyCode::Char('2') => Some(AppAction::PalleteShow("goto 2".to_owned())),
                KeyCode::Char('3') => Some(AppAction::PalleteShow("goto 3".to_owned())),
                KeyCode::Char('4') => Some(AppAction::PalleteShow("goto 4".to_owned())),
                KeyCode::Char('5') => Some(AppAction::PalleteShow("goto 5".to_owned())),
                KeyCode::Char('6') => Some(AppAction::PalleteShow("goto 6".to_owned())),
                KeyCode::Char('7') => Some(AppAction::PalleteShow("goto 7".to_owned())),
                KeyCode::Char('8') => Some(AppAction::PalleteShow("goto 8".to_owned())),
                KeyCode::Char('9') => Some(AppAction::PalleteShow("goto 9".to_owned())),
                _ => None,
            });

        // ---- command keybindings
        hndl.keybinds(AppContext::Command)
            .add(
                Keybind::default()
                    .code(KeyCode::Left)
                    .action(AppAction::PalleteGotoPrev),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Right)
                    .action(AppAction::PalleteGotoNext),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Home)
                    .action(AppAction::PalleteGotoStart),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::End)
                    .action(AppAction::PalleteGotoEnd),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Backspace)
                    .action(AppAction::PalleteDeletePrev),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Delete)
                    .action(AppAction::PalleteDeleteNext),
            )
            // change selection
            .add(
                Keybind::default()
                    .code(KeyCode::Up)
                    .action(AppAction::PalleteSelectPrevious),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Down)
                    .action(AppAction::PalleteSelectNext),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Char('p'))
                    .ctrl()
                    .action(AppAction::PalleteSelectPrevious),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Char('n'))
                    .ctrl()
                    .action(AppAction::PalleteSelectNext),
            )
            // enter esc
            .add(
                Keybind::default()
                    .code(KeyCode::Enter)
                    .action(AppAction::PalleteInsertSelectedOrCommit),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Esc)
                    .action(AppAction::PalleteDeselectOrDismiss),
            )
            // insert characters
            .fallback(|event| {
                if let KeyCode::Char(c) = event.code {
                    Some(AppAction::PalleteInsert(c))
                } else {
                    None
                }
            });

        // ---- sheet keybindings
        hndl.keybinds(AppContext::Sheet)
            // q and esc
            .add(
                Keybind::default()
                    .char('q')
                    .action(AppAction::TableDismissModal),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Esc)
                    .action(AppAction::TableDismissModal),
            )
            // shift up down j k
            .add(
                Keybind::default()
                    .code(KeyCode::Up)
                    .shift()
                    .action(AppAction::SheetScrollUp),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Down)
                    .shift()
                    .action(AppAction::SheetScrollDown),
            )
            .add(
                Keybind::default()
                    .char('K')
                    .action(AppAction::SheetScrollUp),
            )
            .add(
                Keybind::default()
                    .char('J')
                    .action(AppAction::SheetScrollDown),
            );

        // ---- search keybindings
        hndl.keybinds(AppContext::Search)
            // left right home end backspace delete
            .add(
                Keybind::default()
                    .code(KeyCode::Left)
                    .action(AppAction::SearchGotoPrev),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Right)
                    .action(AppAction::SearchGotoNext),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Home)
                    .action(AppAction::SearchGotoStart),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::End)
                    .action(AppAction::SearchGotoEnd),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Backspace)
                    .action(AppAction::SearchDeletePrev),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Delete)
                    .action(AppAction::SearchDeleteNext),
            )
            // enter esc
            .add(
                Keybind::default()
                    .code(KeyCode::Enter)
                    .action(AppAction::SearchCommit),
            )
            .add(
                Keybind::default()
                    .code(KeyCode::Esc)
                    .action(AppAction::SearchRollback),
            )
            // insert characters
            .fallback(|event| {
                if let KeyCode::Char(c) = event.code {
                    Some(AppAction::SearchInsert(c))
                } else {
                    None
                }
            });

        hndl
    }
}
