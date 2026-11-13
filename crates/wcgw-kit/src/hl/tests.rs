//! Highlight groups, colorschemes and `:hi`, through the public `Theme` interface.

use ratatui::style::{Color, Modifier};

use super::*;

fn hi(theme: &mut Theme, line: &str) -> Result<Option<String>, String> {
    let mut words = line.split_whitespace();
    let group = words.next().unwrap_or_default();
    let attrs: Vec<(String, String)> = words
        .map(|word| {
            let (key, value) = word.split_once('=').unwrap_or((word, ""));
            (key.to_owned(), value.to_owned())
        })
        .collect();
    theme.highlight(group, &attrs)
}

fn named(name: &str) -> Theme {
    let mut theme = Theme::new();
    theme.colorscheme(name).expect(name);
    theme
}

const HANDLE: Group = Group::new("AppHandle");

#[test]
fn the_default_theme_is_tokyonight_moon() {
    let theme = Theme::default();

    assert_eq!(theme.name(), "tokyonight-moon");
    let normal = theme.style(Group::NORMAL);
    assert_eq!(normal.fg, Some(Color::Rgb(0xc8, 0xd3, 0xf5)));
    assert_eq!(normal.bg, None, "the terminal's own");
}

#[test]
fn every_built_in_colorscheme_defines_every_standard_group() {
    for name in Theme::new().colorscheme_names() {
        let mut theme = Theme::new();
        theme.colorscheme(&name).expect(&name);
        for &group in Group::STANDARD {
            let defined = theme.specs.contains_key(&group) || theme.links.contains_key(&group);
            assert!(defined, "{name} leaves out {}", group.name());
        }
    }
}

#[test]
fn tokyonight_leaves_the_background_to_the_terminal_floats_too_as_its_transparent_options_do() {
    for name in [
        "tokyonight-night",
        "tokyonight-storm",
        "tokyonight-moon",
        "tokyonight-day",
    ] {
        let theme = named(name);

        assert_eq!(theme.style(Group::NORMAL).bg, None, "{name}");
        assert_eq!(theme.style(Group::TAB_LINE_FILL).bg, None, "{name}");
        assert_eq!(
            theme.style(Group::NORMAL_FLOAT).bg,
            None,
            "{name}: floats too"
        );
        assert_eq!(theme.style(Group::FLOAT_BORDER).bg, None, "{name}");
        assert!(
            theme.style(Group::PMENU_SEL).bg.is_some(),
            "{name}: what's chosen in a float still shows"
        );
    }
}

#[test]
fn colorschemes_are_found_by_name_and_tokyonight_means_night() {
    let day = named("tokyonight-day");
    assert_ne!(
        day.style(Group::NORMAL).fg,
        named("tokyonight-night").style(Group::NORMAL).fg
    );
    assert_eq!(
        named("tokyonight").style(Group::NORMAL),
        named("tokyonight-night").style(Group::NORMAL)
    );
    let mut theme = Theme::new();
    assert_eq!(
        theme.colorscheme("solarized"),
        Err("E185: Cannot find color scheme 'solarized'".to_owned())
    );
    assert_eq!(theme.name(), "tokyonight-moon", "unchanged");
}

#[test]
fn links_are_followed() {
    let theme = Theme::new();

    assert_eq!(
        theme.style(Group::CUR_SEARCH),
        theme.style(Group::INC_SEARCH)
    );
}

#[test]
fn an_application_s_group_takes_its_default_link_until_told_otherwise() {
    let mut theme = Theme::new();
    theme.define(HANDLE, Some(Group::IDENTIFIER));

    assert_eq!(theme.style(HANDLE), theme.style(Group::IDENTIFIER));
    assert_eq!(
        hi(&mut theme, "AppHandle"),
        Ok(Some("AppHandle      xxx links to Identifier".to_owned()))
    );

    assert_eq!(hi(&mut theme, "apphandle guifg=#010203"), Ok(None));
    assert_eq!(theme.style(HANDLE).fg, Some(Color::Rgb(1, 2, 3)));

    theme.clear();
    assert_eq!(
        theme.style(HANDLE),
        theme.style(Group::IDENTIFIER),
        "default links survive :hi clear"
    );
    theme.colorscheme("default").expect("default");
    assert_eq!(
        theme.style(HANDLE),
        theme.style(Group::IDENTIFIER),
        "and colorschemes"
    );
}

#[test]
fn an_application_s_colours_for_a_colorscheme_apply_whenever_it_loads() {
    let mut theme = Theme::new();
    theme.define(HANDLE, Some(Group::IDENTIFIER));

    theme
        .add_overrides("tokyonight-night", "hi AppHandle guifg=#7dcfff")
        .expect("valid");
    assert_eq!(
        theme.style(HANDLE),
        theme.style(Group::IDENTIFIER),
        "not loaded"
    );

    theme.colorscheme("tokyonight-night").expect("night");
    assert_eq!(theme.style(HANDLE).fg, Some(Color::Rgb(0x7d, 0xcf, 0xff)));
    theme.colorscheme("tokyonight-storm").expect("storm");
    assert_eq!(theme.style(HANDLE), theme.style(Group::IDENTIFIER));
}

#[test]
fn overrides_for_the_loaded_colorscheme_apply_at_once() {
    let mut theme = Theme::new();
    theme.define(HANDLE, None);

    theme
        .add_overrides("tokyonight-moon", "hi AppHandle gui=bold")
        .expect("valid");

    assert!(theme.style(HANDLE).add_modifier.contains(Modifier::BOLD));
}

#[test]
fn colorschemes_of_files_are_found_and_shadow_built_in_ones() {
    let mut theme = Theme::new();
    theme.add_colorscheme(
        "mine",
        "\" mine\nhi clear\nhi Normal guifg=#102030\nhi link Visual Search\n",
    );

    theme.colorscheme("mine").expect("added");

    assert_eq!(theme.name(), "mine");
    assert_eq!(
        theme.style(Group::NORMAL).fg,
        Some(Color::Rgb(0x10, 0x20, 0x30))
    );
    assert_eq!(theme.style(Group::VISUAL), theme.style(Group::SEARCH));
    assert!(theme.colorscheme_names().contains(&"mine".to_owned()));
}

#[test]
fn hi_sets_colors_and_attributes_keeping_the_rest() {
    let mut theme = Theme::default();
    let before = theme.style(Group::STATUS_LINE);

    assert_eq!(
        hi(&mut theme, "statusline guifg=#ff0000 gui=bold,italic"),
        Ok(None)
    );

    let after = theme.style(Group::STATUS_LINE);
    assert_eq!(after.fg, Some(Color::Rgb(0xff, 0, 0)));
    assert_eq!(after.bg, before.bg);
    assert!(
        after
            .add_modifier
            .contains(Modifier::BOLD | Modifier::ITALIC)
    );
    assert_eq!(hi(&mut theme, "StatusLine bg=NONE ctermbg=0"), Ok(None));
    assert_eq!(theme.style(Group::STATUS_LINE).bg, None);
}

#[test]
fn hi_with_only_a_group_shows_it() {
    let mut theme = named("tokyonight-night");
    let _ = hi(&mut theme, "Visual gui=reverse");

    assert_eq!(
        hi(&mut theme, "Visual"),
        Ok(Some(
            "Visual         xxx gui=reverse guibg=#283457".to_owned()
        ))
    );
    assert_eq!(
        hi(&mut theme, "CurSearch"),
        Ok(Some("CurSearch      xxx links to IncSearch".to_owned()))
    );
}

#[test]
fn hi_reports_vim_errors_and_changes_nothing() {
    let mut theme = Theme::default();
    let normal = theme.style(Group::NORMAL);

    assert_eq!(
        hi(&mut theme, "Nope guifg=#fff000"),
        Err("E411: Highlight group not found: Nope".to_owned())
    );
    assert_eq!(
        hi(&mut theme, "Normal guifg=#000000 guibg=#zzz"),
        Err("E254: Cannot allocate color #zzz".to_owned())
    );
    assert_eq!(
        hi(&mut theme, "Normal frob=1"),
        Err("E423: Illegal argument: frob=1".to_owned())
    );
    assert_eq!(
        hi(&mut theme, "Normal gui=wobbly"),
        Err("E418: Illegal value: wobbly".to_owned())
    );
    assert_eq!(theme.style(Group::NORMAL), normal);
}

#[test]
fn hi_clear_goes_back_to_the_colorscheme() {
    let mut theme = named("tokyonight-storm");
    let original = theme.style(Group::VISUAL);
    let _ = hi(&mut theme, "Visual guibg=#000000");

    theme.clear();

    assert_eq!(theme.style(Group::VISUAL), original);
    assert_eq!(theme.name(), "tokyonight-storm");
}

#[test]
fn named_terminal_colors_are_accepted() {
    let mut theme = named("default");

    assert_eq!(
        hi(&mut theme, "ErrorMsg guifg=Red guibg=darkgray"),
        Ok(None)
    );

    assert_eq!(theme.style(Group::ERROR_MSG).fg, Some(Color::Red));
    assert_eq!(theme.style(Group::ERROR_MSG).bg, Some(Color::DarkGray));
}

#[test]
fn hi_link_draws_one_group_as_another_until_unlinked() {
    let mut theme = Theme::new();

    theme.link("Folded", Some("ErrorMsg")).expect("known");
    assert_eq!(theme.style(Group::FOLDED), theme.style(Group::ERROR_MSG));

    theme.link("Folded", None).expect("known");
    assert_ne!(theme.style(Group::FOLDED), theme.style(Group::ERROR_MSG));
    assert_eq!(
        theme.link("Folded", Some("Nope")),
        Err("E411: Highlight group not found: Nope".to_owned())
    );
}
