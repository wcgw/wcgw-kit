//! The editor around an application, through a small one: what the kit does on its own, and the seams an
//! application plugs into (D19). devnull's suite covers the editor at length; this one keeps the kit honest alone.

use std::time::Duration;

use super::*;
use crate::app::Msg;
use crate::doc::{Line, Span};
use crate::hl::Group;
use crate::testing::Harness;

/// What toy text refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Thing {
    Page(String),
    Link(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ToyMsg {
    Found(String, Vec<String>),
    /// What the toy was busy with is done: it quits.
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ToyEffect {
    Search(String),
    Load(String),
}

/// A toy application: pages by name, and what it heard.
#[derive(Debug, Default)]
struct Toy {
    shown: Vec<String>,
    opened: Vec<Thing>,
    edited: Vec<Result<String, String>>,
    answers: Vec<char>,
    typed: Vec<String>,
    unread: u64,
    /// The window `:open` shows pages in, when not the current one: a main window beside a sidebar.
    open_in: Option<WindowId>,
    /// Whether `:q` waits for the toy to quit by itself.
    busy: bool,
}

impl App for Toy {
    type Target = Thing;
    type Msg = ToyMsg;
    type Effect = ToyEffect;

    fn update(&mut self, kit: &mut Kit<Self>, msg: ToyMsg) -> Effects<Self> {
        match msg {
            ToyMsg::Found(term, names) => {
                if let Some(picker) = kit.picker_mut() {
                    let choices = names
                        .into_iter()
                        .map(|name| Choice {
                            label: vec![(name.clone(), Group::NORMAL_FLOAT)],
                            text: name.clone(),
                            pick: Pick::Target(Thing::Page(name)),
                        })
                        .collect();
                    picker.found(&term, choices, "nothing");
                }
                Vec::new()
            }
            ToyMsg::Done => vec![Effect::Quit],
        }
    }

    fn open(&mut self, kit: &mut Kit<Self>, target: Thing) -> Result<Effects<Self>, String> {
        self.opened.push(target.clone());
        match target {
            Thing::Page(name) => self.edit(kit, &name),
            Thing::Link(url) => Ok(vec![Effect::OpenUrl(url)]),
        }
    }

    fn edit(&mut self, kit: &mut Kit<Self>, name: &str) -> Result<Effects<Self>, String> {
        if !name.starts_with("page/") {
            return Err(format!("E94: No matching buffer for {name}"));
        }
        if let Some(window) = self.open_in {
            kit.focus_window(window);
        }
        let id = kit
            .find_buffer(name)
            .unwrap_or_else(|| kit.add_buffer(name, page(name), true));
        kit.show_buffer(id);
        Ok(Vec::new())
    }

    fn quit(&mut self, _kit: &mut Kit<Self>) -> crate::app::Quitting {
        if self.busy {
            crate::app::Quitting::Later
        } else {
            crate::app::Quitting::Now
        }
    }

    fn edit_names(&self, _kit: &Kit<Self>) -> Vec<String> {
        vec!["page/a".to_owned(), "page/b".to_owned()]
    }

    fn shown(&mut self, kit: &mut Kit<Self>, buffer: BufferId) -> Effects<Self> {
        let name = kit.buffer(buffer).name().to_owned();
        self.shown.push(name.clone());
        vec![Effect::App(ToyEffect::Load(name))]
    }

    fn edited(&mut self, _kit: &mut Kit<Self>, saved: Result<String, String>) -> Effects<Self> {
        self.edited.push(saved);
        Vec::new()
    }
}

/// A page called `name`: its name, then a line linking to another page.
fn page(name: &str) -> Document<Thing> {
    let mut doc = Document::new();
    doc.push_back(
        "head",
        vec![
            Line::text(name),
            Line::Text(vec![
                Span::new("next", Group::UNDERLINED)
                    .with_target(Thing::Page(format!("{name}/next"))),
                Span::new(" ", Group::NORMAL),
                Span::new("web", Group::UNDERLINED)
                    .with_target(Thing::Link("https://example.com".into())),
            ]),
        ],
    );
    doc
}

/// The toy on `page/home`, `width` × `height`, with its commands.
fn toy(width: u16, height: u16) -> Harness<Toy> {
    let mut kit = Kit::new("page/home", page("page/home"), true);
    kit.add_command(
        Command::new("ask", 2, |_: &mut Toy, kit, _| {
            kit.confirm("Sure? (y/n)", None, |toy: &mut Toy, _, answer| {
                toy.answers.push(answer);
                Vec::new()
            });
            Ok(Vec::new())
        })
        .help("ask something"),
    );
    kit.add_command(
        Command::new("name", 4, |_: &mut Toy, kit, _| {
            kit.prompt("Name: ", false, |toy: &mut Toy, _, name| {
                toy.typed.push(name);
                Vec::new()
            });
            Ok(Vec::new())
        })
        .complete(|_, _, before, word| {
            if before.is_empty() {
                ["ada", "alan", "grace"]
                    .into_iter()
                    .filter(|name| name.starts_with(word))
                    .map(str::to_owned)
                    .collect()
            } else {
                Vec::new()
            }
        }),
    );
    kit.add_command(Command::new("compose", 4, |_: &mut Toy, _, line| {
        Ok(vec![Effect::Edit(crate::app::Edit::new(line.args))])
    }));
    kit.add_command(
        Command::new("forced", 3, |_: &mut Toy, kit, line| {
            kit.info(format!("forced {}", line.bang));
            Ok(Vec::new())
        })
        .bang(),
    );
    kit.add_picker("people", |_, _| {
        Picker::search("People", "Type a name", |_, _, term| {
            vec![Effect::App(ToyEffect::Search(term))]
        })
    });
    kit.add_segment(Segment::new(Side::Right, 1, |toy: &Toy, _| {
        (toy.unread > 0).then(|| format!("{} unread", toy.unread))
    }));
    kit.add_segment(Segment::new(Side::Right, 0, |_: &Toy, _| {
        Some("@me".to_owned())
    }));
    Harness::new(Toy::default(), kit, width, height)
}

fn cmdline(toy: &Harness<Toy>) -> String {
    toy.screen().rows.last().cloned().unwrap_or_default()
}

fn statusline(toy: &Harness<Toy>) -> String {
    let rows = toy.screen().rows;
    rows[rows.len() - 2].clone()
}

#[test]
fn one_listed_buffer_shows_no_tabline_and_one_statusline_for_all() {
    let toy = toy(40, 8);

    let rows = toy.screen().rows;
    assert_eq!(
        rows[0], "page/home",
        "the text from the top row: no tabline"
    );
    assert!(rows[6].starts_with(" NORMAL  page/home"), "{rows:#?}");
    assert_eq!(rows[7], "");
}

#[test]
fn a_second_listed_buffer_brings_the_tabline() {
    let mut toy = toy(40, 8);

    let _ = toy.press("j0<CR>");

    let rows = toy.screen().rows;
    assert_eq!(rows[0], " page/home  page/home/next", "{rows:#?}");
    assert_eq!(rows[1], "page/home/next");
}

#[test]
fn open_hands_the_target_to_the_application_and_c_t_comes_back() {
    let mut toy = toy(40, 8);

    let effects = toy.press("j0<CR>");

    assert_eq!(toy.app.opened, [Thing::Page("page/home/next".into())]);
    assert_eq!(toy.app.shown, ["page/home/next"], "the application heard");
    assert_eq!(
        effects,
        [Effect::App(ToyEffect::Load("page/home/next".into()))]
    );
    let _ = toy.press("<C-t>");
    assert_eq!(toy.kit.buffer(toy.kit.current_buffer()).name(), "page/home");
    assert_eq!(
        toy.screen().cursor,
        (0, 2),
        "where it left: past the tabline, row 2"
    );
    assert_eq!(
        toy.press("w<CR>"),
        [Effect::OpenUrl("https://example.com".into())]
    );
}

#[test]
fn edit_asks_the_application_for_names_no_buffer_has() {
    let mut toy = toy(40, 8);

    let _ = toy.press(":e page/x<CR>");
    assert_eq!(toy.kit.buffer(toy.kit.current_buffer()).name(), "page/x");
    let _ = toy.press(":e nope<CR>");
    assert_eq!(cmdline(&toy), "E94: No matching buffer for nope");
}

#[test]
fn commands_are_found_by_abbreviation_and_refuse_a_bang_they_do_not_take() {
    let mut toy = toy(60, 8);

    let _ = toy.press(":for!<CR>");
    assert_eq!(cmdline(&toy), "forced true");
    let _ = toy.press(":as!<CR>");
    assert_eq!(cmdline(&toy), "E477: No ! allowed");
    let _ = toy.press(":a<CR>");
    assert_eq!(cmdline(&toy), "E492: Not an editor command: a");
}

#[test]
#[should_panic(expected = ":edit is the kit's")]
fn an_application_cannot_take_a_name_of_the_kit_s() {
    let mut kit: Kit<Toy> = Kit::new("x", Document::new(), true);
    kit.add_command(Command::new("edit", 1, |_, _, _| Ok(Vec::new())));
}

#[test]
fn tab_completes_the_application_s_commands_and_their_arguments() {
    let toy = toy(60, 8);

    let names = toy.kit.ex_completions(&toy.app, "nam");
    assert_eq!(names.candidates, ["name"]);
    let arguments = toy.kit.ex_completions(&toy.app, "name a");
    assert_eq!(arguments.candidates, ["ada", "alan"]);
    assert_eq!(arguments.start, 5);
    let edits = toy.kit.ex_completions(&toy.app, "e page/");
    assert_eq!(edits.candidates, ["page/a", "page/b", "page/home"]);
    let pickers = toy.kit.ex_completions(&toy.app, "pick ");
    assert_eq!(pickers.candidates, ["buffers", "people"]);
}

#[test]
fn a_confirmation_hears_the_next_key() {
    let mut toy = toy(40, 8);

    let _ = toy.press(":ask<CR>");
    assert_eq!(cmdline(&toy), "Sure? (y/n)");
    let _ = toy.press("y");
    let _ = toy.press(":ask<CR><Esc>");

    assert_eq!(toy.app.answers, ['y', '\0']);
}

#[test]
fn a_prompt_hears_the_line_typed() {
    let mut toy = toy(40, 8);

    let _ = toy.press(":name<CR>Ada<CR>");

    assert_eq!(toy.app.typed, ["Ada"]);
}

#[test]
fn a_prompt_may_complete_what_is_typed_with_tab() {
    let mut toy = toy(40, 8);
    toy.kit.prompt_completing(
        "Move to: ",
        |_, _, typed| {
            ["Archive", "Arts", "Inbox"]
                .into_iter()
                .filter(|name| name.starts_with(typed))
                .map(str::to_owned)
                .collect()
        },
        |toy: &mut Toy, _, name| {
            toy.typed.push(name);
            Vec::new()
        },
    );

    let _ = toy.press("Ar<Tab><Tab>");
    assert_eq!(cmdline(&toy), "Move to: Arts");
    let _ = toy.press("<CR>");

    assert_eq!(toy.app.typed, ["Arts"]);
    let _ = toy.press(":name<CR>A<Tab><CR>");
    assert_eq!(
        toy.app.typed,
        ["Arts", "A"],
        "a prompt without completions ignores it"
    );
}

#[test]
fn what_the_editor_saved_goes_to_the_application() {
    let mut toy = toy(40, 8);

    assert_eq!(
        toy.press(":comp draft<CR>"),
        [Effect::Edit(crate::app::Edit::new("draft"))]
    );
    let _ = toy.update(Msg::Edited(Ok("saved".into())));

    assert_eq!(toy.app.edited, [Ok("saved".to_owned())]);
}

#[test]
fn shell_and_bang_run_a_command_with_the_terminal_and_alone_ask_which() {
    let mut toy = toy(40, 8);
    let date = || Effect::Shell(crate::app::Shell::new("date").wait());

    assert_eq!(toy.press(":shell date<CR>"), [date()]);
    assert_eq!(toy.press(":!date<CR>"), [date()]);
    assert_eq!(toy.press("!"), []);
    assert_eq!(cmdline(&toy), "!", "asks which");
    assert_eq!(toy.press("date<CR>"), [date()]);
    // Nothing typed: the user's shell, which comes back by itself.
    let effects = toy.press("!<CR>");
    assert!(
        matches!(effects.as_slice(), [Effect::Shell(shell)] if !shell.wait && shell.stdin.is_none()),
        "{effects:?}"
    );
}

#[test]
fn a_shell_command_that_fails_says_so_as_vim_does() {
    let mut toy = toy(40, 8);

    let _ = toy.update(Msg::Shelled(Ok(0)));
    assert_eq!(cmdline(&toy), "");
    let _ = toy.update(Msg::Shelled(Ok(2)));
    assert_eq!(cmdline(&toy), "shell returned 2");
}

#[test]
fn a_searching_picker_asks_once_typing_pauses_and_lists_what_comes_back() {
    let mut toy = toy(60, 16);

    let _ = toy.press(":pick people<CR>");
    assert!(
        toy.screen()
            .rows
            .iter()
            .any(|row| row.contains("Type a name"))
    );
    let typed = toy.press("gr");
    assert!(
        typed.iter().all(|effect| !matches!(effect, Effect::App(_))),
        "not yet"
    );
    let searched = toy.advance(Duration::from_millis(200));
    assert_eq!(searched, [Effect::App(ToyEffect::Search("gr".into()))]);

    let _ = toy.send(ToyMsg::Found("gr".into(), vec!["page/grace".into()]));
    assert!(
        toy.screen()
            .rows
            .iter()
            .any(|row| row.contains("page/grace"))
    );
    let _ = toy.press("<CR>");

    assert_eq!(toy.app.opened, [Thing::Page("page/grace".into())]);
    assert_eq!(
        toy.kit.buffer(toy.kit.current_buffer()).name(),
        "page/grace"
    );
}

#[test]
fn segments_of_the_lowest_priority_go_first_when_the_statusline_is_short() {
    let mut toy = toy(60, 8);
    toy.app.unread = 3;

    assert!(
        statusline(&toy).contains("3 unread  @me  1,1"),
        "{}",
        statusline(&toy)
    );
    // " NORMAL  page/home" and a blank take 19 cells; "3 unread  1,1  All " 19 more.
    let _ = toy.update(Msg::Resize {
        width: 40,
        height: 8,
    });
    assert!(
        statusline(&toy).contains("3 unread  1,1"),
        "{}",
        statusline(&toy)
    );
    let _ = toy.update(Msg::Resize {
        width: 30,
        height: 8,
    });
    assert!(!statusline(&toy).contains("unread"), "{}", statusline(&toy));
}

#[test]
fn help_lists_the_application_s_commands_with_the_kit_s() {
    let mut toy = toy(60, 40);

    let _ = toy.press(":help<CR>G");

    let rows = toy.screen().rows;
    assert!(
        rows.iter()
            .any(|row| row.contains(":ask") && row.contains("ask something")),
        "{rows:#?}"
    );
}

/// A buffer with a picture of `cols` × `rows` cells from `source` on its first line.
fn pictured(toy: &mut Harness<Toy>, source: &str, cols: u16, rows: u16) {
    let mut doc = Document::new();
    doc.push_back(
        "picture",
        vec![Line::Image {
            rows,
            cols,
            source: source.to_owned(),
        }],
    );
    *toy.kit.current_doc_mut() = doc;
}

#[test]
fn with_graphics_off_pictures_are_neither_asked_for_nor_drawn() {
    let mut toy = toy(40, 8);
    let _ = toy.kit.set_graphics(Graphics::HalfBlocks);
    pictured(&mut toy, "a.png", 20, 2);

    assert!(
        toy.press("j")
            .iter()
            .any(|effect| matches!(effect, Effect::Picture { .. }))
    );
    let _ = toy.press(":set graphics=off<CR>");
    let effects = toy.press("k");

    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Picture { .. }))
    );
    assert!(
        !toy.screen().rows[0].contains("loading"),
        "{:?}",
        toy.screen().rows
    );
}

#[test]
fn a_picture_prepared_but_no_longer_wanted_deletes_what_it_staged() {
    let dir = tempfile::tempdir().expect("dir");
    let staged = dir.path().join("staged.rgba");
    std::fs::write(&staged, b"pixels").expect("write");
    let mut toy = toy(40, 8);

    let effects = toy.update(Msg::Picture {
        source: "never-asked.png".into(),
        cols: 4,
        rows: 2,
        result: Ok(Prepared::Kitty {
            escapes: "<sent>".into(),
            staged: Some(staged.clone()),
            bytes: 64,
        }),
    });

    assert!(!effects.contains(&Effect::Graphics("<sent>".into())));
    assert!(!staged.exists());
}

#[test]
fn pictures_out_of_view_go_once_they_take_more_than_the_budget() {
    let mut toy = toy(40, 8);
    let _ = toy.kit.set_graphics(Graphics::Kitty);
    let big = 100 << 20;
    let mut sent = Vec::new();
    let mut arrived = Vec::new();
    for (source, line) in [("one.png", "a"), ("two.png", "b")] {
        pictured(&mut toy, source, 4, 2);
        let effects = toy.press("j");
        let kitty = effects.iter().find_map(|effect| match effect {
            Effect::Picture { kitty, .. } => *kitty,
            _ => None,
        });
        let id = kitty.expect("asked for, with an id");
        arrived = toy.update(Msg::Picture {
            source: source.to_owned(),
            cols: 4,
            rows: 2,
            result: Ok(Prepared::Kitty {
                escapes: format!("<{line}>"),
                staged: None,
                bytes: big,
            }),
        });
        sent.push(id);
    }

    // Both together take 200 MiB, over the 128 MiB budget: the one out of view goes as the second arrives.
    let effects = arrived;
    assert!(
        effects.contains(&Effect::Graphics(wcgw_kit_gfx::kitty_delete(sent[0]))),
        "{effects:?}"
    );
    assert!(!effects.contains(&Effect::Graphics(wcgw_kit_gfx::kitty_delete(sent[1]))));
}

/// The toy with a composer on its page that sends what is written as a link, and completes `@` names.
fn composing(width: u16, height: u16) -> Harness<Toy> {
    let mut toy = toy(width, height);
    let id = toy.kit.current_buffer();
    let composer = Composer::new("message")
        .always_shown()
        .on_send(|toy: &mut Toy, _, _, text| {
            toy.typed.push(text);
            Vec::new()
        })
        .completing(
            |_: &Toy, _, _, word| match word.text.strip_prefix('@') {
                Some(name) => ["ada", "alan", "grace"]
                    .into_iter()
                    .filter(|known| known.starts_with(name))
                    .map(|known| Suggestion {
                        label: vec![(format!("@{known}"), Group::NORMAL)],
                        insert: format!("@{known} "),
                        key: known.to_owned(),
                    })
                    .collect(),
                None => Vec::new(),
            },
            |toy: &mut Toy, _, _, chosen| {
                toy.typed.push(format!("picked {}", chosen.key));
                Vec::new()
            },
        );
    toy.kit.set_composer(id, composer);
    toy.kit.keymap_mut().bind(
        &[crate::keymap::Mode::Normal],
        "i",
        "startinsert",
        "write",
        crate::keymap::Topic::new(75, "Writing"),
    );
    toy
}

#[test]
fn a_composer_docks_under_its_buffer_and_insert_mode_types_into_it() {
    let mut toy = composing(40, 10);

    let rows = toy.screen().rows;
    assert_eq!(rows[6], "─".repeat(40), "its border: {rows:#?}");
    assert_eq!(rows[7], "message", "the placeholder");
    let _ = toy.press("ihello");
    let screen = toy.screen();
    assert_eq!(screen.rows[7], "hello");
    assert!(screen.rows[8].starts_with(" INSERT "), "{:#?}", screen.rows);
    assert_eq!(screen.cursor, (5, 7));

    let _ = toy.press("<CR>");
    assert_eq!(toy.app.typed, ["hello"], "sent");
    assert_eq!(
        toy.screen().rows[7],
        " message",
        "emptied, the cursor before the placeholder"
    );
    let _ = toy.press("<Esc>");
    assert!(toy.screen().rows[8].starts_with(" NORMAL "));
}

#[test]
fn alt_enter_breaks_the_line_and_the_composer_grows() {
    let mut toy = composing(40, 12);

    let _ = toy.press("ione<A-CR>two");

    let rows = toy.screen().rows;
    assert_eq!(rows[8..10], ["one", "two"], "{rows:#?}");
    assert_eq!(
        toy.kit
            .composer(toy.kit.current_buffer())
            .map(Composer::text),
        Some("one\ntwo".into())
    );
}

#[test]
fn words_complete_from_what_the_application_suggests() {
    let mut toy = composing(40, 12);

    let _ = toy.press("ihi @a");
    let rows = toy.screen().rows;
    assert!(rows.iter().any(|row| row.contains("@ada")), "{rows:#?}");
    assert!(rows.iter().any(|row| row.contains("@alan")), "{rows:#?}");
    let _ = toy.press("<C-n><CR>");

    let id = toy.kit.current_buffer();
    assert_eq!(
        toy.kit.composer(id).map(Composer::text),
        Some("hi @alan ".into())
    );
    assert_eq!(toy.app.typed, ["picked alan"]);
    let _ = toy.press("@z");
    assert!(
        toy.kit
            .composer(id)
            .is_some_and(|composer| composer.suggestions().is_empty())
    );
}

#[test]
fn ctrl_x_ctrl_e_edits_the_text_in_the_editor() {
    let mut toy = composing(40, 10);

    let effects = toy.press("idraft<C-x><C-e>");
    assert_eq!(effects, [Effect::Edit(crate::app::Edit::new("draft"))]);
    let _ = toy.update(Msg::Edited(Ok("edited\n".into())));

    let id = toy.kit.current_buffer();
    assert_eq!(
        toy.kit.composer(id).map(Composer::text),
        Some("edited".into())
    );
    assert!(
        toy.app.edited.is_empty(),
        "the kit's, not the application's"
    );
}

#[test]
fn pasting_in_insert_mode_inserts_and_startinsert_needs_a_composer() {
    let mut toy = composing(40, 10);

    let _ = toy.press("i");
    let _ = toy.update(Msg::Paste("pasted".into()));
    let id = toy.kit.current_buffer();
    assert_eq!(
        toy.kit.composer(id).map(Composer::text),
        Some("pasted".into())
    );

    let mut plain = self::toy(40, 10);
    let _ = plain.press(":startinsert<CR>");
    assert_eq!(cmdline(&plain), "Nothing to write in here");
}

#[test]
fn a_sidebar_keeps_its_width_and_a_split_takes_its_share() {
    let mut toy = toy(60, 10);
    let main = toy.kit.current_window();
    let side = toy.kit.add_buffer("side", page("side"), false);
    let thread = toy.kit.add_buffer("thread", page("thread"), true);

    let sidebar = toy
        .kit
        .open_window(Placement::FarLeft, Size::Cells(12), side);
    assert_eq!(toy.kit.current_window(), sidebar);
    toy.kit.focus_window(main);
    let beside = toy
        .kit
        .open_window(Placement::Right, Size::Percent(50), thread);

    let rows = toy.screen().rows;
    // 60 columns: 12 for the sidebar and a separator, then 47 shared, half of them the thread's.
    assert_eq!(rows[1].find('│'), Some(12), "{rows:#?}");
    assert!(rows[1].starts_with("side"), "{rows:#?}");
    assert!(rows[1].contains("page/home"), "{rows:#?}");
    assert!(
        rows[1].ends_with("thread") || rows[1].contains("│thread"),
        "{rows:#?}"
    );
    assert_eq!(toy.kit.window_buffer(beside), Some(thread));
    assert_eq!(toy.kit.windows_showing(side), [sidebar]);
    assert_eq!(toy.kit.window_ids(), [sidebar, main, beside]);
    assert!(toy.kit.close_window_id(beside));
    assert_eq!(toy.kit.window_ids(), [sidebar, main]);
}

#[test]
fn a_winbar_takes_the_first_row_of_the_windows_showing_its_buffer() {
    let mut toy = toy(40, 8);
    let id = toy.kit.current_buffer();

    toy.kit
        .buffer_mut(id)
        .set_winbar(Some(vec![(" Home ".to_owned(), Group::TITLE)]));

    let screen = toy.screen();
    assert_eq!(screen.rows[0], " Home", "{:#?}", screen.rows);
    assert_eq!(screen.rows[1], "page/home");
    assert_eq!(screen.cursor, (0, 1));

    toy.kit.buffer_mut(id).set_winbar(Some(vec![(
        " Home · limited to from:ann and more besides".to_owned(),
        Group::TITLE,
    )]));
    assert_eq!(
        toy.screen().rows[0],
        " Home · limited to from:ann and more bes",
        "cut at the edge, not wrapped at a word"
    );
}

#[test]
fn a_winbar_may_hold_a_picture_asked_for_and_drawn_as_the_text_s() {
    let mut toy = toy(40, 8);
    let _ = toy.kit.set_graphics(Graphics::Kitty);
    let id = toy.kit.current_buffer();

    toy.kit.buffer_mut(id).set_winbar_spans(Some(vec![
        Span::picture("avatar.png", 2),
        Span::new(" Ada", Group::TITLE),
    ]));
    let effects = toy.press("j");

    let asked = effects.iter().find_map(|effect| match effect {
        Effect::Picture {
            source,
            cols,
            rows,
            kitty,
        } => Some((source.clone(), *cols, *rows, *kitty)),
        _ => None,
    });
    let Some((source, 2, 1, Some(_))) = asked else {
        panic!("{effects:?}");
    };
    assert_eq!(source, "avatar.png");
    let _ = toy.update(Msg::Picture {
        source,
        cols: 2,
        rows: 1,
        result: Ok(Prepared::Kitty {
            escapes: "<sent>".into(),
            staged: None,
            bytes: 64,
        }),
    });
    let winbar = &toy.screen().rows[0];
    assert!(winbar.contains('\u{10EEEE}'), "placeholders: {winbar:?}");
    assert!(winbar.ends_with(" Ada"), "{winbar:?}");
}

#[test]
fn a_buffer_that_follows_its_end_starts_there_and_stays_while_the_cursor_does() {
    let mut toy = toy(40, 8);
    let mut doc = Document::new();
    for n in 1..=10 {
        doc.push_back(
            format!("m{n}").as_str(),
            vec![Line::text(format!("message {n}"))],
        );
    }
    let chat = toy.kit.add_buffer("chat", doc, true);
    toy.kit.buffer_mut(chat).set_follows_end(true);

    toy.kit.show_buffer(chat);
    let _ = toy.update(Msg::FocusGained);
    assert_eq!(toy.kit.cursor_row_text(), "message 10");

    toy.kit
        .buffer_mut(chat)
        .doc_mut()
        .push_back("m11", vec![Line::text("message 11")]);
    let _ = toy.update(Msg::FocusGained);
    assert_eq!(toy.kit.cursor_row_text(), "message 11", "followed");

    let _ = toy.press("k");
    toy.kit
        .buffer_mut(chat)
        .doc_mut()
        .push_back("m12", vec![Line::text("message 12")]);
    let _ = toy.update(Msg::FocusGained);
    assert_eq!(
        toy.kit.cursor_row_text(),
        "message 10",
        "left where it was read"
    );
}

fn mouse(toy: &mut Harness<Toy>, mouse: crate::key::Mouse, column: u16, row: u16) -> Effects<Toy> {
    toy.update(Msg::Mouse { mouse, column, row })
}

#[test]
fn a_click_focuses_its_window_and_puts_the_cursor_there() {
    let mut toy = toy(60, 10);
    let other = toy.kit.add_buffer("other", page("other"), false);
    let main = toy.kit.current_window();
    let side = toy
        .kit
        .open_window(Placement::FarLeft, Size::Cells(20), other);
    toy.kit.focus_window(main);

    let _ = mouse(&mut toy, crate::key::Mouse::Left, 7, 1);

    assert_eq!(toy.kit.current_window(), side);
    assert_eq!(toy.screen().cursor, (7, 1), "on `web`");
    assert_eq!(
        toy.kit.target_under_cursor(),
        Some(Thing::Link("https://example.com".into()))
    );
}

#[test]
fn a_click_runs_what_leftmouse_is_mapped_to() {
    let mut toy = toy(40, 8);
    let _ = toy.press(":nnoremap <lt>LeftMouse> :open<lt>CR><CR>");

    let effects = mouse(&mut toy, crate::key::Mouse::Left, 6, 1);

    assert_eq!(effects, [Effect::OpenUrl("https://example.com".into())]);
}

#[test]
fn a_double_click_opens_what_it_lands_on_and_a_click_alone_does_not() {
    let mut toy = toy(40, 8);

    let clicked = mouse(&mut toy, crate::key::Mouse::Left, 6, 1);
    let double = mouse(&mut toy, crate::key::Mouse::DoubleLeft, 6, 1);

    assert_eq!(clicked, []);
    assert_eq!(double, [Effect::OpenUrl("https://example.com".into())]);
}

#[test]
fn the_wheel_scrolls_the_window_under_the_pointer() {
    let mut toy = toy(40, 6);
    let mut doc = Document::new();
    for n in 1..=20 {
        doc.push_back(
            format!("l{n}").as_str(),
            vec![Line::text(format!("line {n}"))],
        );
    }
    *toy.kit.current_doc_mut() = doc;

    let _ = mouse(&mut toy, crate::key::Mouse::WheelDown, 2, 2);

    assert_eq!(toy.screen().rows[0], "line 4");
    let _ = mouse(&mut toy, crate::key::Mouse::WheelUp, 2, 2);
    assert_eq!(toy.screen().rows[0], "line 1");
}

#[test]
fn a_click_on_the_tabline_shows_that_buffer() {
    let mut toy = toy(60, 8);
    let _ = toy.press(":e page/x<CR>");
    assert_eq!(toy.screen().rows[0], " page/home  page/x");

    let _ = mouse(&mut toy, crate::key::Mouse::Left, 3, 0);

    assert_eq!(toy.kit.buffer(toy.kit.current_buffer()).name(), "page/home");
}

#[test]
fn a_double_click_on_a_picker_s_item_opens_it() {
    let mut toy = toy(60, 16);
    let _ = toy.press(":e page/x<CR>:pick buffers<CR>");
    let rows = toy.screen().rows;
    let row = rows
        .iter()
        .position(|row| row.contains("page/x"))
        .expect("listed");
    let row = u16::try_from(row).expect("a row");

    let _ = mouse(&mut toy, crate::key::Mouse::DoubleLeft, 30, row);

    assert_eq!(toy.kit.buffer(toy.kit.current_buffer()).name(), "page/x");
}

#[test]
fn the_mouse_is_captured_from_the_start_and_set_mouse_lets_it_go() {
    let mut toy = toy(40, 8);
    let Harness { app, kit, .. } = &mut toy;

    assert_eq!(kit.start(app), [Effect::CaptureMouse(true)]);
    assert_eq!(toy.press(":set mouse=<CR>"), [Effect::CaptureMouse(false)]);
    assert_eq!(mouse(&mut toy, crate::key::Mouse::Left, 6, 1), []);
    assert_eq!(toy.screen().cursor, (0, 0), "ignored");
    assert_eq!(toy.press(":set mouse=a<CR>"), [Effect::CaptureMouse(true)]);
}

#[test]
fn a_span_s_own_colours_go_over_its_group_s() {
    use ratatui::style::{Color, Style};
    let mut toy = toy(40, 8);
    let mut doc = Document::new();
    doc.push_back(
        "a",
        vec![Line::Text(vec![
            Span::new("red", Group::UNDERLINED).with_style(Style::new().fg(Color::Rgb(255, 0, 0))),
        ])],
    );
    *toy.kit.current_doc_mut() = doc;

    let style = toy.style_at(0, 0);

    assert_eq!(style.fg, Some(Color::Rgb(255, 0, 0)), "the span's own");
    assert_eq!(
        style.add_modifier,
        toy.kit.style(Group::UNDERLINED).add_modifier,
        "the rest the group's"
    );
}

#[test]
fn opening_from_a_sidebar_into_another_window_leaves_no_tag_there() {
    let mut toy = toy(60, 10);
    let main = toy.kit.current_window();
    let side = toy.kit.add_buffer("side", page("page/side"), false);
    let _ = toy
        .kit
        .open_window(Placement::FarLeft, Size::Cells(20), side);
    toy.app.open_in = Some(main);

    let _ = toy.press("j0<CR>");
    assert_eq!(toy.kit.current_window(), main);
    assert_eq!(
        toy.kit.buffer(toy.kit.current_buffer()).name(),
        "page/side/next"
    );
    let _ = toy.press("<C-t>");

    assert_eq!(
        cmdline(&toy),
        "E73: Tag stack empty",
        "nothing to go back to here"
    );
    assert_eq!(
        toy.kit.buffer(toy.kit.current_buffer()).name(),
        "page/side/next"
    );
}

#[test]
fn a_click_up_a_buffer_that_follows_its_end_stays_where_it_lands() {
    let mut toy = toy(40, 8);
    let mut doc = Document::new();
    for n in 1..=4 {
        doc.push_back(
            format!("m{n}").as_str(),
            vec![Line::text(format!("message {n}"))],
        );
    }
    let chat = toy.kit.add_buffer("chat", doc, true);
    toy.kit.buffer_mut(chat).set_follows_end(true);
    toy.kit.show_buffer(chat);
    let _ = toy.update(Msg::FocusGained);

    let row = toy
        .screen()
        .rows
        .iter()
        .position(|row| row == "message 2")
        .expect("shown");
    let _ = mouse(
        &mut toy,
        crate::key::Mouse::Left,
        1,
        u16::try_from(row).expect("a row"),
    );

    assert_eq!(toy.kit.cursor_row_text(), "message 2");
}

#[test]
fn a_sidebar_split_in_rows_keeps_its_width_and_its_windows_their_rows() {
    let mut toy = toy(60, 12);
    let side = toy.kit.add_buffer("side", page("side"), false);
    let _ = toy
        .kit
        .open_window(Placement::FarLeft, Size::Cells(20), side);

    let _ = toy.press(":sp<CR>");

    let rows = toy.screen().rows;
    assert_eq!(rows[0].find('│'), Some(20), "{rows:#?}");
    assert!(
        rows[0].starts_with("side"),
        "the new window has rows: {rows:#?}"
    );
    let below = rows
        .iter()
        .skip(1)
        .position(|row| row.starts_with("side"))
        .expect("both");
    assert!(below > 0, "{rows:#?}");
    assert_eq!(rows[below + 1].find('│'), Some(20), "{rows:#?}");
}

/// The toy on a page of three items of two lines each.
fn items(width: u16, height: u16) -> Harness<Toy> {
    let mut toy = toy(width, height);
    let mut doc = Document::new();
    for n in 1..=3 {
        doc.push_back(
            format!("i{n}").as_str(),
            vec![
                Line::text(format!("item {n}")),
                Line::text(format!("more {n}")),
            ],
        );
    }
    *toy.kit.current_doc_mut() = doc;
    toy
}

#[test]
fn cursorline_highlights_the_cursor_s_row_or_its_whole_item() {
    let mut toy = items(40, 10);
    let id = toy.kit.current_buffer();
    let line = toy.kit.style(Group::CURSOR_LINE).bg;
    let _ = toy.press("jj");

    toy.kit.buffer_mut(id).set_cursorline(CursorLine::Row);
    assert_eq!(toy.style_at(0, 2).bg, line, "the row");
    assert_ne!(toy.style_at(0, 3).bg, line);

    toy.kit.buffer_mut(id).set_cursorline(CursorLine::Item);
    assert_eq!(toy.style_at(10, 2).bg, line, "the item, across the window");
    assert_eq!(toy.style_at(0, 3).bg, line);
    assert_ne!(toy.style_at(0, 1).bg, line, "not the item before");
}

#[test]
fn an_application_reads_and_places_the_cursor_of_any_window() {
    let mut toy = items(60, 10);
    let main = toy.kit.current_window();
    let side = toy.kit.add_buffer("side", page("page/side"), false);
    let sidebar = toy
        .kit
        .open_window(Placement::FarLeft, Size::Cells(20), side);

    let _ = toy.press("j0");
    assert_eq!(
        toy.kit.targets_in(sidebar)[0],
        Some(&Thing::Page("page/side/next".into()))
    );
    assert!(toy.kit.at_end_of(sidebar));
    assert!(!toy.kit.at_end_of(main));
    assert_eq!(toy.kit.text_width(side), 20);
    assert_eq!(
        toy.kit.window_text_area(main).map(|area| area.width),
        Some(39)
    );

    let layout_end = Cursor::on_block("i3");
    toy.kit.set_cursor(main, layout_end);
    toy.kit.focus_window(main);
    assert_eq!(toy.kit.cursor_row_text(), "item 3");
    assert_eq!(toy.kit.current_window(), main);
}

#[test]
fn defaults_bound_to_keys_run_them_and_folds_answer_the_application() {
    let mut toy = items(40, 10);
    toy.kit.keymap_mut().bind_keys(
        &[crate::keymap::Mode::Normal],
        "J",
        "jj",
        "two down",
        crate::keymap::Topic::new(75, "Toy"),
    );

    let _ = toy.press("J");
    assert_eq!(toy.kit.cursor_row_text(), "item 2");

    let mut doc = Document::new();
    doc.push_back(
        "f",
        crate::doc::Content {
            lines: vec![Line::text("summary"), Line::text("hidden")],
            folds: vec![crate::doc::Fold {
                lines: 1..2,
                summary: Line::text("folded"),
                closed: false,
            }],
            target: None,
        },
    );
    *toy.kit.current_doc_mut() = doc;
    let _ = toy.press("gg");
    assert!(!toy.kit.on_closed_fold());
    toy.kit.fold(None, false);
    assert_eq!(toy.screen().rows[1], "folded");
    let _ = toy.press("j");
    assert!(toy.kit.on_closed_fold());
    toy.kit.fold(None, true);
    assert_eq!(toy.screen().rows[1], "hidden");
    assert!(!toy.kit.on_closed_fold());
}

#[test]
fn quitting_waits_for_an_application_that_asks_but_not_with_a_bang() {
    let mut toy = toy(40, 8);
    let quits = |effects: &Effects<Toy>| effects.iter().any(|e| matches!(e, Effect::Quit));

    assert!(quits(&toy.press(":q<CR>")));
    toy.app.busy = true;
    assert!(!quits(&toy.press(":q<CR>")));
    assert!(!quits(&toy.press(":qa<CR>")));
    assert!(quits(&toy.press(":qa!<CR>")));
}

#[test]
fn showing_the_card_again_enters_it_to_scroll_and_open_from() {
    let mut toy = toy(40, 16);
    toy.kit
        .add_command(Command::new("card", 4, |_: &mut Toy, kit, _| {
            let mut card = Document::new();
            let lines = (0..12)
                .map(|n| {
                    let span = Span::new(format!("line {n}"), Group::NORMAL);
                    Line::Text(vec![if n == 7 {
                        span.with_target(Thing::Link("seven".into()))
                    } else {
                        span
                    }])
                })
                .collect::<Vec<_>>();
            card.push_back("card", crate::doc::Content::from(lines));
            kit.show_hover(card);
            Ok(Vec::new())
        }));
    toy.kit.keymap_mut().bind_keys(
        &[crate::keymap::Mode::Normal],
        "K",
        ":card<CR>",
        "a card",
        crate::keymap::Topic::new(75, "Toy"),
    );
    let shows =
        |toy: &Harness<Toy>, text: &str| toy.screen().rows.iter().any(|row| row.contains(text));

    let _ = toy.press("K");
    assert!(shows(&toy, "line 0"));
    let _ = toy.press("j");
    assert!(!shows(&toy, "line 0"), "the next key takes it away");

    let _ = toy.press("KKG");
    assert!(
        shows(&toy, "line 11") && !shows(&toy, "line 0"),
        "{:?}",
        toy.screen().rows
    );
    let _ = toy.press("gg");
    assert!(shows(&toy, "line 0"));
    let _ = toy.press("q");
    assert!(!shows(&toy, "line 0"), "q leaves it");

    let _ = toy.press("KKjjjjjjj<CR>");
    assert_eq!(toy.app.opened, [Thing::Link("seven".into())]);
    assert!(!shows(&toy, "line 0"));
}

#[test]
fn quitting_frees_the_pictures_however_it_comes() {
    for busy in [false, true] {
        let mut toy = toy(40, 8);
        let _ = toy.kit.set_graphics(Graphics::Kitty);
        pictured(&mut toy, "one.png", 4, 2);
        let id = toy
            .press("j")
            .iter()
            .find_map(|effect| match effect {
                Effect::Picture { kitty, .. } => *kitty,
                _ => None,
            })
            .expect("asked for");
        let _ = toy.update(Msg::Picture {
            source: "one.png".into(),
            cols: 4,
            rows: 2,
            result: Ok(Prepared::Kitty {
                escapes: "<sent>".into(),
                staged: None,
                bytes: 64,
            }),
        });
        toy.app.busy = busy;

        let mut effects = toy.press(":qa<CR>");
        if busy {
            effects.extend(toy.send(ToyMsg::Done));
        }

        let deleted = Effect::Graphics(wcgw_kit_gfx::kitty_delete(id));
        assert_eq!(
            effects.iter().filter(|effect| **effect == deleted).count(),
            1,
            "busy {busy}: {effects:?}"
        );
        assert_eq!(effects.last(), Some(&Effect::Quit));
    }
}

#[test]
fn pictures_in_view_are_asked_for_from_the_start_and_graphics_says_how() {
    let mut toy = toy(40, 8);
    let _ = toy.kit.set_graphics(Graphics::HalfBlocks);
    pictured(&mut toy, "a.png", 20, 2);

    let effects = toy.kit.start(&mut toy.app);

    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::Picture { .. }))
    );
    assert_eq!(toy.kit.graphics(), Graphics::HalfBlocks);
    let _ = toy.press(":set graphics=off<CR>");
    assert_eq!(toy.kit.graphics(), Graphics::Off);
}

#[test]
fn a_segment_may_have_a_colour_and_a_buffer_a_new_name() {
    let mut toy = toy(60, 8);
    toy.kit.add_segment(
        Segment::new(Side::Left, 2, |_: &Toy, _| Some("down".to_owned()))
            .with_group(Group::ERROR_MSG),
    );
    let id = toy.kit.current_buffer();
    toy.kit.buffer_mut(id).set_name("page/home (3)");

    let status = statusline(&toy);
    assert!(status.contains("page/home (3)  down"), "{status}");
    let x = u16::try_from(status.find("down").expect("shown")).expect("a column");
    assert_eq!(toy.style_at(x, 6).fg, toy.kit.style(Group::ERROR_MSG).fg);
}

#[test]
fn a_picker_s_verbs_act_on_the_choice_and_its_border_says_them() {
    let mut toy = toy(60, 16);
    let _ = toy.press(":e page/x<CR>");
    toy.kit.add_picker("pages", |_, kit| {
        let choices = kit
            .listed()
            .into_iter()
            .map(|id| Choice {
                label: vec![(kit.buffer(id).name().to_owned(), Group::NORMAL_FLOAT)],
                text: kit.buffer(id).name().to_owned(),
                pick: Pick::Target(Thing::Link(kit.buffer(id).name().to_owned())),
            })
            .collect();
        Picker::list("Pages", choices, "none")
            .verb("<C-y>", "copy", |_, _, pick| match pick {
                Pick::Target(Thing::Link(name)) => vec![Effect::Copy(name)],
                _ => Vec::new(),
            })
            .verb("<C-d>", "drop", |_, kit, pick| {
                let Some(picker) = kit.picker_mut() else {
                    return Vec::new();
                };
                let kept = ["page/home", "page/x"]
                    .into_iter()
                    .map(|name| Choice {
                        label: vec![(name.to_owned(), Group::NORMAL_FLOAT)],
                        text: name.to_owned(),
                        pick: Pick::Target(Thing::Link(name.to_owned())),
                    })
                    .filter(|choice| choice.pick != pick)
                    .collect();
                picker.set_choices(kept);
                Vec::new()
            })
            .verb("<C-o>", "open", |_, kit, _| {
                kit.close_picker();
                Vec::new()
            })
    });

    let _ = toy.press(":pick pages<CR>");
    assert!(
        toy.screen()
            .rows
            .iter()
            .any(|row| row.contains("<C-y> copy"))
    );
    let prompt = toy
        .screen()
        .rows
        .iter()
        .position(|row| row.contains("> "))
        .expect("its query");
    let _ = mouse(
        &mut toy,
        crate::key::Mouse::WheelDown,
        30,
        u16::try_from(prompt + 1).expect("row"),
    );

    assert_eq!(toy.press("<C-y>"), [Effect::Copy("page/x".into())]);
    let open = |toy: &Harness<Toy>| toy.screen().rows.iter().any(|row| row.contains("Pages"));
    assert!(open(&toy), "a verb leaves the picker open");
    let _ = toy.press("<C-d>");
    assert!(
        !toy.screen()
            .rows
            .iter()
            .any(|row| row.contains("│  page/x")),
        "it changed the choices: {:?}",
        toy.screen().rows
    );
    assert_eq!(toy.press("<C-y>"), [Effect::Copy("page/home".into())]);
    let _ = toy.press("<C-o>");
    assert!(!open(&toy), "unless it closes it");
}

#[test]
fn the_application_may_take_keys_hear_words_and_hear_insert_mode_end() {
    let mut toy = composing(40, 10);
    let id = toy.kit.current_buffer();
    let composer = Composer::new("message")
        .on_key(|toy: &mut Toy, kit, id, key| {
            (key.code == crate::key::Code::Up && kit.composer(id).is_some_and(Composer::is_empty))
                .then(|| {
                    toy.typed.push("up".into());
                    Vec::new()
                })
        })
        .on_word(|_: &mut Toy, _, _, word| {
            if word.text.starts_with('@') {
                vec![Effect::App(ToyEffect::Search(word.text.to_owned()))]
            } else {
                Vec::new()
            }
        })
        .on_leave(|toy: &mut Toy, _, _| {
            toy.typed.push("left".into());
            Vec::new()
        });
    toy.kit.set_composer(id, composer);

    let _ = toy.press("i<Up>");
    assert_eq!(toy.app.typed, ["up"]);
    let effects = toy.press("@a");
    assert_eq!(
        effects.last(),
        Some(&Effect::App(ToyEffect::Search("@a".into())))
    );
    let _ = toy.press("<Up><Esc>");
    assert_eq!(
        toy.app.typed,
        ["up", "left"],
        "Up went to the text once it had some"
    );
}
